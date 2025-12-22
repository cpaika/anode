# Helm Chart Linting Guide

This guide explains how to lint the Anode Helm chart to ensure it follows best practices and doesn't contain errors.

## Prerequisites

- [Helm](https://helm.sh/docs/intro/install/) 3.x or later installed

## Running Helm Lint

### Basic Lint

Run the basic lint command from the chart directory:

```bash
cd deploy/helm/anode
helm lint .
```

This will check for:
- Chart.yaml validity
- Required fields presence
- Template syntax errors
- Values.yaml structure
- Common Kubernetes resource issues

### Lint with Values Files

Test with different values configurations:

```bash
# Test with production values
helm lint . -f values-production.yaml

# Test with development values
helm lint . -f values-development.yaml

# Test with minimal values
helm lint . -f values-minimal.yaml
```

### Lint with Strict Mode

Enable strict mode to catch warnings as errors:

```bash
helm lint . --strict
```

### Lint with Debug Output

Get detailed output for troubleshooting:

```bash
helm lint . --debug
```

## Template Validation

### Render Templates Locally

Preview what will be deployed without installing:

```bash
# Render all templates
helm template my-release . > rendered.yaml

# Render with specific values
helm template my-release . -f values-production.yaml > rendered-prod.yaml
```

### Dry-run Installation

Test the installation without actually deploying:

```bash
helm install my-release . --dry-run --debug
```

## Using the Validation Script

The chart includes a validation script that checks a running installation:

```bash
# After installation, validate the deployment
./validate.sh

# Validate in a specific namespace
./validate.sh -n my-namespace -r my-release

# Verbose output
./validate.sh -v
```

## Common Issues and Fixes

### Issue: Duplicate Resources

**Error**: `Error: INSTALLATION FAILED: rendered manifests contain a resource that already exists`

**Fix**: Check for duplicate resource definitions across templates. Ensure conditional logic (if statements) prevents creating duplicate resources.

### Issue: Invalid Template Syntax

**Error**: `Error: parse error at (...): unexpected "{{"`

**Fix**: Ensure proper escaping of template delimiters:
```yaml
# Wrong
description: "Value is {{ $value }}"

# Correct
description: "Value is {{ "{{" }} $value {{ "}}" }}"
```

### Issue: Missing Required Fields

**Error**: `[ERROR] Chart.yaml: missing required field: description`

**Fix**: Ensure all required fields are present in Chart.yaml:
```yaml
apiVersion: v2
name: anode
description: Distributed object storage for small clusters
type: application
version: 0.1.0
appVersion: "0.1.0"
```

### Issue: Invalid Values Reference

**Error**: `Error: template: ...: executing "..." at <.Values.nonexistent>: nil pointer evaluating interface {}.nonexistent`

**Fix**: Ensure all referenced values exist in values.yaml or use default values:
```yaml
# Use default if value doesn't exist
{{ .Values.optional.field | default "default-value" }}

# Check if value exists before using
{{- if .Values.optional.field }}
value: {{ .Values.optional.field }}
{{- end }}
```

## Best Practices Checklist

Before committing changes, ensure:

- [ ] `helm lint .` passes without errors
- [ ] `helm lint . --strict` passes without warnings
- [ ] Templates render correctly with `helm template`
- [ ] All values files lint successfully
- [ ] Chart follows [Helm best practices](https://helm.sh/docs/chart_best_practices/)
- [ ] Templates include proper labels using `_helpers.tpl`
- [ ] Resources have conditional creation where appropriate
- [ ] NOTES.txt provides useful installation information
- [ ] README.md is up to date

## Automated Linting

### In CI/CD

Add to your CI/CD pipeline:

```yaml
# Example GitHub Actions
- name: Lint Helm Chart
  run: |
    helm lint deploy/helm/anode
    helm lint deploy/helm/anode --strict
    helm template test deploy/helm/anode > /dev/null
```

### Pre-commit Hook

Create `.git/hooks/pre-commit`:

```bash
#!/bin/bash
cd deploy/helm/anode
helm lint . --strict || exit 1
echo "Helm lint passed!"
```

## Schema Validation

The chart includes a JSON schema (`values.schema.json`) for validating values:

```bash
# Helm automatically validates against schema during lint
helm lint .

# Manual validation with custom values
helm lint . -f my-values.yaml
```

## Additional Resources

- [Helm Lint Documentation](https://helm.sh/docs/helm/helm_lint/)
- [Helm Chart Testing](https://github.com/helm/chart-testing)
- [Helm Best Practices](https://helm.sh/docs/chart_best_practices/)
- [Chart Template Guide](https://helm.sh/docs/chart_template_guide/)

## Quick Reference

```bash
# Common linting workflow
cd deploy/helm/anode

# 1. Basic lint
helm lint .

# 2. Strict lint
helm lint . --strict

# 3. Test all values files
for f in values*.yaml; do
  echo "Testing with $f"
  helm lint . -f "$f"
done

# 4. Render templates
helm template test . > /tmp/rendered.yaml

# 5. Validate rendered output
kubectl apply --dry-run=client -f /tmp/rendered.yaml
```

## Troubleshooting

If linting fails:

1. Check the error message carefully
2. Review the specific template mentioned in the error
3. Verify the corresponding value in values.yaml
4. Test with `helm template` to see rendered output
5. Consult the [Helm documentation](https://helm.sh/docs/)
6. Check similar charts for examples

For persistent issues, run with debug output:

```bash
helm lint . --debug 2>&1 | tee lint-debug.log
```
