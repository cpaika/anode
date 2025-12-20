{{/*
Expand the name of the chart.
*/}}
{{- define "anode.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "anode.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "anode.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "anode.labels" -}}
helm.sh/chart: {{ include "anode.chart" . }}
{{ include "anode.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "anode.selectorLabels" -}}
app.kubernetes.io/name: {{ include "anode.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "anode.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "anode.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the headless service name
*/}}
{{- define "anode.headlessServiceName" -}}
{{- printf "%s-headless" (include "anode.fullname" .) }}
{{- end }}

{{/*
Create the config map name
*/}}
{{- define "anode.configMapName" -}}
{{- printf "%s-config" (include "anode.fullname" .) }}
{{- end }}

{{/*
Create the secret name
*/}}
{{- define "anode.secretName" -}}
{{- printf "%s-secret" (include "anode.fullname" .) }}
{{- end }}

{{/*
Get the image tag
*/}}
{{- define "anode.imageTag" -}}
{{- .Values.image.tag | default .Chart.AppVersion }}
{{- end }}

{{/*
Get the image
*/}}
{{- define "anode.image" -}}
{{- printf "%s:%s" .Values.image.repository (include "anode.imageTag" .) }}
{{- end }}

{{/*
Generate S3 secret key if not provided
*/}}
{{- define "anode.s3SecretKey" -}}
{{- if .Values.s3Credentials.admin.secretKey }}
{{- .Values.s3Credentials.admin.secretKey }}
{{- else }}
{{- randAlphaNum 32 }}
{{- end }}
{{- end }}

{{/*
Generate cluster ID if not provided
*/}}
{{- define "anode.clusterId" -}}
{{- if .Values.config.cluster.clusterId }}
{{- .Values.config.cluster.clusterId }}
{{- else }}
{{- printf "%s-%s" .Release.Name (randAlphaNum 8) }}
{{- end }}
{{- end }}

{{/*
Create the node address
*/}}
{{- define "anode.nodeAddress" -}}
{{- $fullname := include "anode.fullname" . -}}
{{- $headlessService := include "anode.headlessServiceName" . -}}
{{- printf "%s.%s.%s.svc.cluster.local" $fullname $headlessService .Release.Namespace }}
{{- end }}

{{/*
Pod anti-affinity preset
*/}}
{{- define "anode.podAntiAffinity" -}}
podAntiAffinity:
  preferredDuringSchedulingIgnoredDuringExecution:
  - weight: 100
    podAffinityTerm:
      labelSelector:
        matchLabels:
          {{- include "anode.selectorLabels" . | nindent 10 }}
      topologyKey: kubernetes.io/hostname
  - weight: 50
    podAffinityTerm:
      labelSelector:
        matchLabels:
          {{- include "anode.selectorLabels" . | nindent 10 }}
      topologyKey: topology.kubernetes.io/zone
{{- end }}
