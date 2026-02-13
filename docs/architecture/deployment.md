# CIS Deployment Architecture v1.1.6

> **Version**: 1.1.6
> **Last Updated**: 2026-02-13
> **Target Audience**: DevOps Engineers, System Administrators, Platform Operators

---

## Table of Contents

1. [Deployment Overview](#deployment-overview)
2. [Deployment Models](#deployment-models)
3. [System Requirements](#system-requirements)
4. [Installation Methods](#installation-methods)
5. [Configuration Management](#configuration-management)
6. [Network Topology](#network-topology)
7. [Storage Architecture](#storage-architecture)
8. [Security Considerations](#security-considerations)
9. [Monitoring and Observability](#monitoring-and-observability)
10. [Scaling Strategies](#scaling-strategies)
11. [Disaster Recovery](#disaster-recovery)
12. [Maintenance Operations](#maintenance-operations)

---

## Deployment Overview

### Deployment Scenarios

CIS supports multiple deployment scenarios tailored to different use cases:

```
┌─────────────────────────────────────────────────────────────────┐
│                    CIS Deployment Models                     │
├─────────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐             │
│  │   Standalone    │    │     Team       │             │
│  │  (Individual)   │    │  (Small Group)  │             │
│  │                 │    │                 │             │
│  │  • Single user  │    │  • 2-10 users   │             │
│  │  • Local data  │    │  • LAN mDNS    │             │
│  │  • No network  │    │  • Shared memory│             │
│  └─────────────────┘    └─────────────────┘             │
│                                                              │
│  ┌─────────────────┐    ┌─────────────────┐             │
│  │   Distributed   │    │   Enterprise   │             │
│  │  (Global P2P)  │    │  (Managed)     │             │
│  │                 │    │                 │             │
│  │  • 10+ users   │    │  • 100+ users  │             │
│  │  • DHT routing │    │  • Managed infra│             │
│  │  • NAT traversal│    │  • SSO integration│            │
│  └─────────────────┘    └─────────────────┘             │
│                                                              │
└─────────────────────────────────────────────────────────────────┘
```

### Component Distribution

```
┌─────────────────────────────────────────────────────────────────┐
│                  CIS Components                            │
├─────────────────────────────────────────────────────────────────┤
│                                                              │
│  Core Components (Required):                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  cis-core          Core library (Rust crate)       │   │
│  │  cis-node          CLI executable                   │   │
│  │  cis-gui          Desktop GUI (Tauri)            │   │
│  │  SQLite           Embedded database                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                              │
│  Optional Components:                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  P2P Module       libp2p networking (feature flag)│   │
│  │  Matrix Gateway  Federation via Matrix (feature)     │   │
│  │  WASM Runtime     Skill sandboxing (feature)        │   │
│  │  MCP Adapter     Model Context Protocol (feature)    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                              │
│  Skills (Hot-swappable):                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  init-wizard      First-time setup                 │   │
│  │  memory-organizer Memory maintenance               │   │
│  │  ai-executor     AI task execution               │   │
│  │  dag-executor    DAG workflow execution            │   │
│  │  im              Matrix messaging                │   │
│  │  push-client     Push notifications              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Deployment Models

### 1. Standalone Deployment

**Use Case**: Individual users, single-device usage

**Architecture**:
```
┌─────────────────────────────────────┐
│       Single Device              │
│  ┌──────────────────────────┐     │
│  │  CIS Node             │     │
│  │  - Local storage      │     │
│  │  - No networking     │     │
│  │  - Single user       │     │
│  └──────────────────────────┘     │
│                                 │
│  Storage:                        │
│  ~/.cis/data/                   │
│  ├── memory/                    │
│  ├── vector/                    │
│  └── storage/                   │
└─────────────────────────────────────┘
```

**Characteristics**:
- **Networking**: Disabled
- **Data Storage**: Local only
- **Memory Sync**: None
- **Configuration**: User-specific
- **Dependencies**: Minimal (no P2P deps)

**Deployment Steps**:
```bash
# 1. Install CIS
brew install cis  # macOS
# or
cargo install cis-node

# 2. Initialize
cis init

# 3. Run
cis daemon start
```

---

### 2. Team Deployment (Local LAN)

**Use Case**: Small teams (2-10 users) in same office/network

**Architecture**:
```
┌─────────────────────────────────────────────────────┐
│              Local Network (192.168.1.0/24)   │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Node A  │  │  Node B  │  │  Node C  │  │
│  │.1.2      │  │.1.3      │  │.1.4      │  │
│  │          │  │          │  │          │  │
│  │ mDNS     │◀─▶│ mDNS     │◀─▶│ mDNS     │  │
│  │ Discovery│  │ Discovery│  │ Discovery│  │
│  └─────┬────┘  └─────┬────┘  └─────┬────┘  │
│        │              │              │        │
│        └──────────────┴──────────────┘        │
│               │                               │
│               ▼                               │
│        ┌──────────────┐                       │
│        │ Public Memory│                       │
│        │ Sync        │                       │
│        └──────────────┘                       │
│                                                 │
└─────────────────────────────────────────────────────┘
```

**Characteristics**:
- **Networking**: mDNS-based local discovery
- **Data Storage**: Local with optional sync
- **Memory Sync**: Public domain synced across LAN
- **Configuration**: Shared config template
- **Dependencies**: P2P module enabled

**Network Requirements**:
- **Port**: 7677 (P2P listen port)
- **Protocol**: QUIC over UDP
- **Discovery**: mDNS ( multicast, port 5353)
- **Bandwidth**: ~1 Mbps per node for sync

**Deployment Steps**:
```bash
# 1. Install CIS on all nodes
# (on each machine)

# 2. Configure P2P
cat > ~/.cis/config.toml << 'EOF'
[p2p]
enabled = true
listen_port = 7677

[p2p.discovery]
enable_mdns = true
enable_dht = false
EOF

# 3. Start nodes
cis daemon start

# 4. Verify discovery
cis p2p peers
# Should show other LAN nodes
```

---

### 3. Distributed Deployment (Global P2P)

**Use Case**: Distributed teams, remote workers, global scale

**Architecture**:
```
┌─────────────────────────────────────────────────────────────┐
│                     Global P2P Network                │
│                                                         │
│  ┌──────────────┐        ┌──────────────┐           │
│  │ Home Network │        │ Office      │           │
│  │              │        │ Network     │           │
│  │ ┌────────┐    │        │ ┌────────┐  │           │
│  │ │Node A  │    │        │ │Node B  │  │           │
│  │ │NAT: R1│    │        │ │NAT: R2│  │           │
│  │ └───┬────┘    │        │ └───┬────┘  │           │
│  └──────┼─────────┘        └──────┼───────┘           │
│         │                            │                    │
│         └────────┬───────────────────┘                 │
│                  ▼                                  │
│         ┌──────────────┐                             │
│         │  DHT &      │                             │
│         │  Relay      │                             │
│         │  Nodes      │                             │
│         └──────┬───────┘                             │
│                │                                     │
│         ┌──────┴───────┐                             │
│         │ Bootstrap   │                             │
│         │ Nodes      │                             │
│         └─────────────┘                             │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐                 │
│  │ Cloud       │  │ Mobile      │                 │
│  │ Node C      │  │ Node D      │                 │
│  │ (No NAT)   │  │ (Cellular)  │                 │
│  └─────────────┘  └─────────────┘                 │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**Characteristics**:
- **Networking**: P2P with DHT and relay
- **Data Storage**: Local with public memory sync
- **Memory Sync**: Global public memory sync
- **Configuration**: Bootstrap nodes, DHT enabled
- **Dependencies**: Full P2P stack

**Network Requirements**:
- **Port**: 7677 (or UPnP/NAT traversal)
- **Protocol**: QUIC over UDP with relay fallback
- **Discovery**: DHT + mDNS + bootstrap nodes
- **Bandwidth**: ~5 Mbps per node for global sync
- **NAT**: Requires UPnP or manual port forwarding

**Bootstrap Nodes**:
```toml
[p2p]
bootstrap_nodes = [
    "/dns/cis-bootstrap-1.example.com/tcp/7677/p2p/12D3KooW...",
    "/dns/cis-bootstrap-2.example.com/tcp/7677/p2p/12D3KooW...",
    "/ip4/203.0.113.1/tcp/7677/p2p/12D3KooW...",
]
```

**Deployment Steps**:
```bash
# 1. Install CIS
brew install cis

# 2. Configure P2P with DHT
cat > ~/.cis/config.toml << 'EOF'
[p2p]
enabled = true
listen_port = 7677

[p2p.discovery]
enable_mdns = true
enable_dht = true

[p2p.bootstrap]
nodes = [
    "/dns/cis-bootstrap.example.com/tcp/7677/p2p/12D3KooW..."
]
EOF

# 3. Configure NAT (if needed)
# Option A: UPnP (automatic)
# Option B: Port forwarding
#   Forward external port 7677 to internal 7677

# 4. Start node
cis daemon start

# 5. Verify connectivity
cis p2p status
cis p2p peers
```

---

### 4. Enterprise Deployment

**Use Case**: Large organizations, managed infrastructure

**Architecture**:
```
┌─────────────────────────────────────────────────────────────┐
│                  Enterprise Deployment                  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐   │
│  │            Infrastructure Layer                │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐   │   │
│  │  │  LB     │  │ Monitor │  │ Logging │   │   │
│  │  │ (Nginx) │  │ (Prom)  │  │ (ELK)   │   │   │
│  │  └────┬────┘  └─────────┘  └─────────┘   │   │
│  └───────┼───────────────────────────────────────┘   │
│          │                                          │
│  ┌───────┴───────────────────────────────────────┐   │
│  │          CIS Cluster                   │   │
│  │  ┌────────┐  ┌────────┐  ┌────────┐   │   │
│  │  │ Node 1 │  │ Node 2 │  │ Node 3 │   │   │
│  │  │        │  │        │  │        │   │   │
│  │  └───┬────┘  └───┬────┘  └───┬────┘   │   │
│  └──────┼────────────┼────────────┼──────────┘   │
│         │            │            │              │
│         └────────────┴────────────┘              │
│                      │                           │
│  ┌───────────────────┴───────────────────────┐   │
│  │          Shared Storage            │   │
│  │  ┌─────────┐  ┌─────────┐             │   │
│  │  │  NFS    │  │  Object  │             │   │
│  │  │ Storage │  │  Storage │             │   │
│  │  └─────────┘  └─────────┘             │   │
│  └────────────────────────────────────────────┘   │
│                                                 │
│  ┌─────────────────────────────────────────────┐   │
│  │       Enterprise Integration          │   │
│  │  ┌─────────┐  ┌─────────┐             │   │
│  │  │   SSO   │  │   LDAP  │             │   │
│  │  │ (SAML)  │  │ (Auth)  │             │   │
│  │  └─────────┘  └─────────┘             │   │
│  └────────────────────────────────────────────┘   │
│                                                 │
└─────────────────────────────────────────────────────┘
```

**Characteristics**:
- **Networking**: Managed P2P with dedicated relays
- **Data Storage**: Distributed with shared storage backend
- **Memory Sync**: Selective sync based on policy
- **Configuration**: Centralized config management
- **Dependencies**: Full stack + enterprise integrations

**Infrastructure Requirements**:
- **Load Balancer**: Nginx/HAProxy for API distribution
- **Monitoring**: Prometheus + Grafana
- **Logging**: ELK Stack (Elasticsearch, Logstash, Kibana)
- **Storage**: NFS or Object Storage (S3/MinIO)
- **Auth**: SSO (SAML/OIDC) + LDAP
- **Relay Nodes**: 3-5 dedicated relay nodes for NAT traversal

**Deployment Architecture**:
```yaml
# Docker Compose for Enterprise
version: '3.8'

services:
  cis-node-1:
    image: cis:latest
    ports:
      - "7677:7677"
      - "8080:8080"
    volumes:
      - nfs-storage:/data
    environment:
      - CIS_NODE_ID=node-1
      - CIS_BOOTSTRAP_NODES=/dns/relay1/tcp/7677/p2p/...
      - CIS_SSO_PROVIDER=saml
      - CIS_LDAP_URL=ldap://ldap.internal

  cis-node-2:
    image: cis:latest
    # ... similar config

  cis-relay:
    image: cis-relay:latest
    ports:
      - "7677:7677"
    deploy:
      replicas: 3

  prometheus:
    image: prom/prometheus
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana
    ports:
      - "3000:3000"

volumes:
  nfs-storage:
    driver: local
    driver_opts:
      type: nfs
      device: ":/path/to/nfs/share"
```

---

## System Requirements

### Minimum Requirements (Standalone)

| Component | Specification |
|-----------|--------------|
| **OS** | Linux (Ubuntu 20.04+, RHEL 8+), macOS 11+, Windows 10+ |
| **CPU** | 2 cores, x86_64 |
| **RAM** | 4 GB |
| **Storage** | 10 GB free space (SSD recommended) |
| **Network** | Not required (standalone) |

### Recommended Requirements (Team/Distributed)

| Component | Specification |
|-----------|--------------|
| **OS** | Linux (Ubuntu 22.04+, RHEL 9+) |
| **CPU** | 4+ cores, x86_64 |
| **RAM** | 8 GB |
| **Storage** | 50 GB free space (NVMe SSD) |
| **Network** | 10 Mbps uplink, stable connection |
| **Ports** | 7677 (P2P), 5353 (mDNS) |

### Enterprise Requirements

| Component | Specification |
|-----------|--------------|
| **OS** | Linux (Ubuntu 22.04 LTS, RHEL 9) |
| **CPU** | 8+ cores, x86_64 |
| **RAM** | 16 GB+ |
| **Storage** | 200 GB+ NVMe SSD or NFS |
| **Network** | 100 Mbps+ dedicated, low latency |
| **High Availability** | Multiple nodes, load balancer |
| **Backup** | Off-site backup storage |

### Disk Space Calculation

```
Storage Breakdown per Node:

├── Core CIS: ~100 MB
│   ├── Binary: ~30 MB
│   ├── Skills: ~50 MB
│   └── Dependencies: ~20 MB
│
├── Memory Data: ~500 MB/year
│   ├── Weekly DBs: 1 MB/week × 54 = 54 MB
│   ├── Archives: 0.5 MB/week × 50 = 25 MB
│   └── Vector Index: ~200 MB
│
├── Logs: ~1 GB/year
│   ├── Request logs: ~100 MB/month
│   └── Debug logs: ~500 MB/month
│
└── P2P Data: ~200 MB
    ├── Peer cache: ~50 MB
    ├── DHT data: ~100 MB
    └── CRDT state: ~50 MB

Total First Year: ~2 GB
Growth Rate: ~2-3 GB/year
```

---

## Installation Methods

### Method 1: Package Manager (Recommended)

**Homebrew (macOS/Linux)**:
```bash
# Add tap
brew tap cis-system/tap

# Install CIS
brew install cis

# Start service
brew services start cis

# Verify installation
cis --version
```

**APT (Debian/Ubuntu)**:
```bash
# Add repository
curl -fsSL https://apt.cis.system/KEY.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cis-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/cis-archive-keyring.gpg] https://apt.cis.system stable main" | sudo tee /etc/apt/sources.list.d/cis.list

# Install
sudo apt update
sudo apt install cis

# Start service
sudo systemctl enable cis
sudo systemctl start cis
```

**YUM (RHEL/CentOS)**:
```bash
# Add repository
sudo yum install -y yum-utils
sudo yum-config-manager --add-repo https://yum.cis.system/cis.repo

# Install
sudo yum install cis

# Start service
sudo systemctl enable cis
sudo systemctl start cis
```

---

### Method 2: Cargo Install

**From Crates.io**:
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install CIS
cargo install cis-node --features full

# Initialize
cis-node init

# Run
cis-node daemon start
```

---

### Method 3: Binary Download

**Direct Download**:
```bash
# Download latest release
wget https://github.com/cis-system/cis/releases/download/v1.1.6/cis-linux-amd64.tar.gz

# Extract
tar -xzf cis-linux-amd64.tar.gz

# Install
sudo install cis /usr/local/bin/
sudo install cis-gui /usr/local/bin/

# Initialize
cis init
```

---

### Method 4: Docker Deployment

**Dockerfile**:
```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features full

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/cis /usr/local/bin/
COPY --from=builder /app/target/release/cis-gui /usr/local/bin/
EXPOSE 7677 8080
CMD ["cis", "daemon", "start"]
```

**Docker Compose**:
```yaml
version: '3.8'

services:
  cis:
    build: .
    container_name: cis-node
    ports:
      - "7677:7677"  # P2P
      - "8080:8080"  # API
    volumes:
      - cis-data:/root/.cis
      - ./config:/root/.cis/config:ro
    environment:
      - RUST_LOG=info
      - CIS_P2P_ENABLED=true
    restart: unless-stopped

volumes:
  cis-data:
```

---

### Method 5: Kubernetes Deployment

**Deployment YAML**:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cis
  namespace: default
spec:
  replicas: 3
  selector:
    matchLabels:
      app: cis
  template:
    metadata:
      labels:
        app: cis
    spec:
      containers:
      - name: cis
        image: cis:v1.1.6
        ports:
        - containerPort: 7677
          name: p2p
        - containerPort: 8080
          name: api
        env:
        - name: CIS_P2P_ENABLED
          value: "true"
        - name: CIS_BOOTSTRAP_NODES
          value: "/dns/cis-relay/tcp/7677/p2p/..."
        volumeMounts:
        - name: data
          mountPath: /root/.cis
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: cis-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: cis-service
spec:
  selector:
    app: cis
  ports:
  - name: p2p
    port: 7677
    targetPort: 7677
  - name: api
    port: 8080
    targetPort: 8080
  type: LoadBalancer
```

---

## Configuration Management

### Configuration Hierarchy

```
Priority (highest to lowest):
1. CLI Arguments (--option value)
2. Environment Variables (CIS_*)
3. Project Config (.cis/project.toml)
4. Global Config (~/.cis/config.toml)
5. Default Values (built-in)
```

### Environment Variables

```bash
# Core
export CIS_DATA_DIR="/var/lib/cis"
export CIS_LOG_LEVEL="info"
export CIS_MAX_CONCURRENT_TASKS="10"

# P2P
export CIS_P2P_ENABLED="true"
export CIS_P2P_LISTEN_PORT="7677"
export CIS_P2P_BOOTSTRAP_NODES="/dns/.../tcp/7677/p2p/..."

# Memory
export CIS_MEMORY_DATA_DIR="/var/lib/cis/memory"
export CIS_MEMORY_HOT_WEEKS="4"
export CIS_MEMORY_TOTAL_WEEKS="54"

# Agent
export CIS_AGENT_DEFAULT="claude"
export CIS_AGENT_MODEL="claude-3-sonnet"

# Enterprise
export CIS_SSO_PROVIDER="saml"
export CIS_SSO_METADATA_URL="https://sso.example.com/metadata"
export CIS_LDAP_URL="ldap://ldap.example.com"
export CIS_LDAP_BASE_DN="dc=example,dc=com"
```

### Configuration Validation

```bash
# Validate configuration
cis config validate

# Test configuration
cis config test

# Show effective configuration
cis config show
```

---

## Network Topology

### Port Usage

| Port | Protocol | Purpose | External Access |
|------|----------|---------|----------------|
| 7677 | QUIC (UDP) | P2P communication | Yes (with NAT) |
| 8080 | HTTP | REST API | Optional |
| 5353 | mDNS (UDP) | Local discovery | No (multicast) |
| 9090 | TCP | Metrics endpoint | Optional |

### Firewall Configuration

```bash
# Allow P2P traffic
sudo ufw allow 7677/udp

# Allow API (if enabled)
sudo ufw allow 8080/tcp

# Allow mDNS (local only)
sudo ufw allow from 192.168.0.0/16 to any port 5353 proto udp

# Check status
sudo ufw status
```

### NAT Traversal Strategies

**Strategy 1: UPnP (Automatic)**
```toml
[p2p.nat]
enable_upnp = true
```

**Strategy 2: Port Forwarding (Manual)**
```
External Port: 7677 → Internal IP: 192.168.1.100:7677
Protocol: UDP
```

**Strategy 3: Relay (Fallback)**
```toml
[p2p.relay]
enabled = true
relay_nodes = [
    "/dns/relay1.cis.system/tcp/7677/p2p/...",
    "/dns/relay2.cis.system/tcp/7677/p2p/...",
]
```

---

## Storage Architecture

### Directory Structure

```
~/.cis/
├── config.toml              # Global configuration
├── data/                   # Application data
│   ├── core/               # Core databases
│   │   ├── core.db        # Main storage
│   │   ├── federation.db  # Matrix federation
│   │   └── conversations.db
│   ├── memory/             # Memory system
│   │   ├── memory-2026-W06.db  # Current week
│   │   ├── memory-2026-W05.db  # Previous week
│   │   ├── index/
│   │   │   ├── hnsw_index.db  # Vector index
│   │   │   └── index_entries
│   │   └── archives/
│   │       ├── 2026-W06.db.gz
│   │       └── ...
│   ├── vector/             # Vector embeddings
│   │   ├── models/
│   │   │   └── all-MiniLM-L6-v2.npy
│   │   └── cache/
│   ├── telemetry/          # Request logs
│   │   └── requests.db
│   └── p2p/              # P2P data
│       ├── peer_cache.db
│       └── dht_data.db
├── skills/                  # User-installed skills
│   ├── custom-skill/
│   └── ...
└── logs/                    # Application logs
    ├── cis.log
    └── cis-error.log
```

### Storage Backend Options

**SQLite (Default)**:
- Embedded, no setup required
- ACID transactions
- Suitable for all deployment sizes

**PostgreSQL (Enterprise)**:
```toml
[storage]
backend = "postgresql"
url = "postgresql://user:pass@localhost/cis"
connection_pool = 10
```

**Shared Storage (Cluster)**:
```toml
[storage]
backend = "nfs"
path = "/mnt/nfs/cis/data"
```

---

## Security Considerations

### Hardening Guidelines

**1. File Permissions**:
```bash
# Restrict CIS data directory
chmod 700 ~/.cis
chmod 600 ~/.cis/config.toml

# Private memory encryption
chmod 600 ~/.cis/data/memory/private.db
```

**2. Network Security**:
```bash
# Use firewall to restrict P2P access
sudo ufw allow from 10.0.0.0/8 to any port 7677 proto udp

# Enable TLS for API
[p2p]
enable_encryption = true
noise_protocol = "Noise_XK"
```

**3. Memory Encryption**:
```toml
[memory]
encryption = true
encryption_key = "${CIS_ENCRYPTION_KEY}"  # From env var
```

**4. Access Control**:
```toml
[network.acl]
default_policy = "deny"
rules = [
    { principal = "did:key:...", resource = "memory:*", action = "read" },
    { principal = "did:key:...", resource = "memory:*", action = "write" },
]
```

### Security Auditing

```bash
# Enable audit logging
[memory]
audit_log = true
audit_log_path = "/var/log/cis/audit.log"

# Monitor access
cis audit log --tail
```

---

## Monitoring and Observability

### Metrics Collection

**Prometheus Integration**:
```toml
[telemetry]
prometheus_enabled = true
prometheus_port = 9090
```

**Key Metrics**:
- `cis_memory_ops_total` - Memory operations count
- `cis_task_duration_seconds` - Task execution time
- `cis_skill_load_duration_seconds` - Skill load time
- `cis_p2p_peer_count` - Connected peers
- `cis_vector_index_size` - Index entries

### Logging

**Log Levels**:
```toml
[core]
log_level = "info"  # error, warn, info, debug, trace
```

**Log Rotation**:
```toml
[logging]
max_file_size = "100MB"
max_files = 10
compression = true
```

### Health Checks

```bash
# Check node health
cis health status

# Expected output:
{
  "status": "healthy",
  "uptime": 1234567,
  "memory_usage": "45%",
  "peer_count": 5,
  "tasks_running": 2
}
```

---

## Scaling Strategies

### Horizontal Scaling (P2P)

**Add more nodes**:
```bash
# Deploy new node with same bootstrap configuration
# Network automatically rebalances
```

**Benefits**:
- Linear scaling for P2P operations
- Improved redundancy
- Load distribution

### Vertical Scaling

**Increase resources**:
```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "1000m"
  limits:
    memory: "4Gi"
    cpu: "4000m"
```

**Benefits**:
- Better single-node performance
- Faster vector search
- More concurrent tasks

### Storage Scaling

**Archive to cold storage**:
```bash
# Move old archives to S3
aws s3 sync ~/.cis/data/memory/archives/ s3://my-bucket/cis-archives/

# Keep only recent archives locally
find ~/.cis/data/memory/archives/ -mtime +180 -delete
```

---

## Disaster Recovery

### Backup Strategy

**Full Backup**:
```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d)
BACKUP_DIR="/backup/cis/$DATE"

# Stop CIS
cis daemon stop

# Backup data
tar -czf "$BACKUP_DIR/cis-data.tar.gz" ~/.cis/data
tar -czf "$BACKUP_DIR/cis-config.tar.gz" ~/.cis/config.toml

# Start CIS
cis daemon start

# Upload to remote
aws s3 cp "$BACKUP_DIR" s3://backup-bucket/cis/ --recursive
```

**Incremental Backup**:
```bash
# Use WAL (Write-Ahead Log)
[storage]
wal_mode = true
wal_checkpoint_interval = "5min"

# Backup only WAL and changes
rsync -av ~/.cis/data/ /backup/cis/data/
```

### Recovery Procedure

**Restore from backup**:
```bash
#!/bin/bash
# restore.sh

BACKUP_DATE=$1

# Stop CIS
cis daemon stop

# Restore data
rm -rf ~/.cis/data
tar -xzf "/backup/cis/$BACKUP_DATE/cis-data.tar.gz" -C ~/

# Restore config
tar -xzf "/backup/cis/$BACKUP_DATE/cis-config.tar.gz" -C ~/

# Start CIS
cis daemon start

# Verify
cis health status
```

---

## Maintenance Operations

### Routine Maintenance

**Daily**:
```bash
# Check health
cis health status

# Review logs
tail -100 ~/.cis/logs/cis.log
```

**Weekly**:
```bash
# Cleanup old logs
cis telemetry cleanup --days 30

# Review storage
du -sh ~/.cis/data/
```

**Monthly**:
```bash
# Update CIS
brew upgrade cis

# Review skill updates
cis skill update --all

# Backup
backup.sh
```

### Zero-Downtime Updates

**Rolling Update (Cluster)**:
```bash
# Update nodes one by one
for node in node1 node2 node3; do
  ssh $node "brew upgrade cis && cis daemon restart"
  # Wait for node to rejoin P2P network
  sleep 30
done
```

---

## Conclusion

CIS deployment architecture provides flexibility for:
- **Individual users**: Simple standalone installation
- **Small teams**: LAN-based deployment with mDNS
- **Distributed teams**: Global P2P network with DHT
- **Enterprise**: Managed infrastructure with HA and monitoring

Key deployment considerations:
1. **Network setup**: Port forwarding, NAT traversal, firewall rules
2. **Storage planning**: Growth rate ~2-3 GB/year
3. **Monitoring**: Metrics, logs, health checks
4. **Backup**: Regular automated backups
5. **Security**: Encryption, ACLs, audit logging

---

**Document Version**: 1.0
**Last Updated**: 2026-02-13
**Authors**: CIS Architecture Team
