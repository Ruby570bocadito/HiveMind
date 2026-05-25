{{- define "hive-colony.name" -}}
{{- default "hive-colony" .Chart.Name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "hive-colony.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default "hive-colony" .Chart.Name }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "hive-colony.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "hive-colony.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- "default" }}
{{- end }}
{{- end }}

{{- define "hive-colony.labels" -}}
helm.sh/chart: {{ include "hive-colony.name" . }}-{{ .Chart.Version }}
app.kubernetes.io/name: {{ include "hive-colony.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "hive-colony.selectorLabels" -}}
app.kubernetes.io/name: {{ include "hive-colony.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
