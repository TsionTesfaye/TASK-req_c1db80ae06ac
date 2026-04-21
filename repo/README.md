# TerraOps

A full-stack offline operations portal for environmental observation, catalog governance, KPI analytics, and talent intelligence. Single-node deployment — one TLS origin serves both the REST API and the Yew SPA with no outbound network dependencies at runtime.

## Architecture & Tech Stack

* **Frontend:** Rust + Yew 0.21 + Trunk + Tailwind CSS (compiled to WebAssembly)
* **Backend:** Rust + Actix-web 4 + Rustls 0.22 + SQLx 0.7
* **Database:** PostgreSQL 16
* **Containerization:** Docker & Docker Compose (required)

## Project Structure

```text
.
├── crates/
│   ├── backend/          # Actix-web REST API + SPA server (port 8443)
│   ├── frontend/         # Yew SPA compiled to WASM by Trunk
│   └── shared/           # Shared DTOs, RBAC, errors, pagination
├── e2e/                  # Playwright end-to-end specs (9 flows)
├── migrations/           # PostgreSQL migration files
├── scripts/              # Bootstrap, TLS cert, seed, and admin helpers
├── docker-compose.yml    # Services: db (PostgreSQL 16) + app
├── Dockerfile.app        # Multi-stage: backend + frontend build → slim runtime
├── Dockerfile.tests      # Test runner: cargo llvm-cov + wasm-bindgen + Playwright
├── run_tests.sh          # Standardized test execution script — MANDATORY
└── README.md
```

## Prerequisites

* [Docker](https://docs.docker.com/get-docker/)
* [Docker Compose](https://docs.docker.com/compose/install/) (v2.1+)

No host-side Rust, Node, or browser installation required. Everything runs inside containers.

## Running the Application

1. **Build and start containers:**

   ```bash
   docker-compose up --build -d
   ```

   Also works with the modern CLI:

   ```bash
   docker compose up --build -d
   ```

   On first boot the container automatically runs database migrations and seeds all demo accounts — no manual setup required.

2. **Access the app:**

   * **Application:** `https://localhost:8443`

   The server uses a dev-only self-signed TLS certificate. Accept the browser warning on first visit.

   * **Health check:** `https://localhost:8443/api/v1/health` → `{"status":"ok"}`

3. **Stop the application:**

   ```bash
   docker-compose down -v
   ```

## Testing

All unit, integration, and end-to-end tests run through a single script. It builds the test image, runs backend cargo tests with line-coverage enforcement, runs the frontend wasm-bindgen-test suite, and runs Playwright flows against the live app.

```bash
chmod +x run_tests.sh
./run_tests.sh
```

Returns exit code `0` on success, non-zero on any failure.

## Seeded Credentials

All demo accounts are seeded automatically on startup. **Sign-in uses `username`, not email.**

| Role | Username | Email | Password |
| :--- | :--- | :--- | :--- |
| **Administrator** | `admin` | `admin@terraops.local` | `TerraOps!2026` |
| **Data Steward** | `steward` | `steward@terraops.local` | `TerraOps!2026` |
| **Analyst** | `analyst` | `analyst@terraops.local` | `TerraOps!2026` |
| **Recruiter** | `recruiter` | `recruiter@terraops.local` | `TerraOps!2026` |
| **Regular User** | `user` | `user@terraops.local` | `TerraOps!2026` |
