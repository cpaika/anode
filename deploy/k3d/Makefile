.PHONY: help cluster deploy test clean rebuild reset logs shell status

# Default target
.DEFAULT_GOAL := help

# Configuration
NAMESPACE ?= anode
RELEASE_NAME ?= anode
IMAGE_TAG ?= latest

# Colors for output
GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m

help: ## Show this help message
	@echo "$(GREEN)Anode K3d Testing Infrastructure$(NC)"
	@echo ""
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-15s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "Environment variables:"
	@echo "  NAMESPACE      Kubernetes namespace (default: anode)"
	@echo "  RELEASE_NAME   Helm release name (default: anode)"
	@echo "  IMAGE_TAG      Docker image tag (default: latest)"

cluster: ## Create K3d cluster
	@echo "$(GREEN)[STEP]$(NC) Creating K3d cluster..."
	./create-cluster.sh

deploy: cluster ## Build, load, and deploy anode to cluster
	@echo "$(GREEN)[STEP]$(NC) Building and deploying anode..."
	./build-and-load.sh
	./deploy-anode.sh

test: deploy ## Run all integration tests
	@echo "$(GREEN)[STEP]$(NC) Running integration tests..."
	./run-tests.sh

rebuild: ## Rebuild and redeploy without recreating cluster
	@echo "$(GREEN)[STEP]$(NC) Rebuilding and redeploying..."
	./build-and-load.sh
	./deploy-anode.sh

reset: clean cluster ## Delete cluster and create fresh one
	@echo "$(GREEN)[STEP]$(NC) Cluster reset complete"

clean: ## Delete K3d cluster and all resources
	@echo "$(GREEN)[STEP]$(NC) Cleaning up..."
	./delete-cluster.sh

logs: ## Show logs from all anode pods
	@echo "$(GREEN)[INFO]$(NC) Showing logs from anode pods..."
	kubectl logs -n $(NAMESPACE) -l app.kubernetes.io/name=anode \
		--all-containers --prefix --tail=100 -f

shell: ## Open shell in first anode pod
	@echo "$(GREEN)[INFO]$(NC) Opening shell in pod $(RELEASE_NAME)-0..."
	kubectl exec -n $(NAMESPACE) -it $(RELEASE_NAME)-0 -- /bin/sh

status: ## Show cluster and deployment status
	@echo "$(GREEN)[INFO]$(NC) Cluster Status:"
	@echo ""
	@echo "Nodes:"
	@kubectl get nodes -o wide || echo "  No cluster running"
	@echo ""
	@echo "Namespaces:"
	@kubectl get namespaces || echo "  No cluster running"
	@echo ""
	@echo "$(GREEN)[INFO]$(NC) Deployment Status:"
	@echo ""
	@echo "Pods:"
	@kubectl get pods -n $(NAMESPACE) -l app.kubernetes.io/name=anode -o wide 2>/dev/null || echo "  No deployment found"
	@echo ""
	@echo "Services:"
	@kubectl get services -n $(NAMESPACE) -l app.kubernetes.io/name=anode 2>/dev/null || echo "  No services found"
	@echo ""
	@echo "StatefulSets:"
	@kubectl get statefulsets -n $(NAMESPACE) -l app.kubernetes.io/name=anode 2>/dev/null || echo "  No StatefulSets found"
	@echo ""
	@echo "PVCs:"
	@kubectl get pvc -n $(NAMESPACE) 2>/dev/null || echo "  No PVCs found"

# Test individual components
test-deployment: ## Run deployment test only
	@./tests/test_deployment.sh

test-scaling: ## Run scaling test only
	@./tests/test_scaling.sh

test-rolling-update: ## Run rolling update test only
	@./tests/test_rolling_update.sh

test-node-failure: ## Run node failure test only
	@./tests/test_node_failure.sh

test-s3: ## Run S3 operations test only
	@./tests/test_s3_operations.sh

test-persistence: ## Run data persistence test only
	@./tests/test_data_persistence.sh

# Port forwarding helpers
port-forward-s3: ## Port forward to S3 API (port 8080)
	@echo "$(GREEN)[INFO]$(NC) Port forwarding to S3 API..."
	@echo "Access S3 API at: http://localhost:8080"
	kubectl port-forward -n $(NAMESPACE) svc/$(RELEASE_NAME) 8080:8080

port-forward-admin: ## Port forward to Admin API (port 8081)
	@echo "$(GREEN)[INFO]$(NC) Port forwarding to Admin API..."
	@echo "Access Admin API at: http://localhost:8081"
	kubectl port-forward -n $(NAMESPACE) svc/$(RELEASE_NAME) 8081:8081

# Development helpers
describe-pod: ## Describe first anode pod
	@kubectl describe pod -n $(NAMESPACE) $(RELEASE_NAME)-0

watch-pods: ## Watch pod status
	@watch kubectl get pods -n $(NAMESPACE) -l app.kubernetes.io/name=anode

events: ## Show recent events
	@kubectl get events -n $(NAMESPACE) --sort-by='.lastTimestamp' | tail -20

# Quick development cycle
dev: rebuild ## Quick rebuild and deploy (alias for rebuild)
	@echo "$(GREEN)[INFO]$(NC) Development deployment complete!"
	@echo ""
	@echo "Next steps:"
	@echo "  make logs           - View logs"
	@echo "  make test           - Run tests"
	@echo "  make shell          - Open shell in pod"
