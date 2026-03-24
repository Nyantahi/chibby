# Pipeline Templates

Chibby's template system lets you create, share, and reuse pipeline configurations across projects. Templates come in two flavors: **full pipelines** (complete multi-stage configurations) and **stage snippets** (individual stages you can add to any pipeline).

## Template Resolution

Templates are loaded from three layers, with higher-priority layers overriding lower ones:

| Priority | Source | Location |
| -------- | ------ | -------- |
| Highest | Project | `<repo>/.chibby/templates/` |
| Medium | User | `~/.chibby/templates/` |
| Lowest | Built-in | Bundled with the app binary |

If a project template has the same name as a built-in template, the project version is used. This lets you customize built-in templates per project or per user without modifying the app.

## Built-in Templates

Chibby ships with 19 built-in templates covering common languages and deployment patterns.

### Full Pipeline Templates (9)

| Template | Category | Description |
| -------- | -------- | ----------- |
| Rust CLI | rust | Format, lint (clippy), test, and release build |
| Rust Library | rust | Test, document, and publish a Rust library |
| Node.js Web App | node | Install, lint, test, build, and deploy |
| Python Django | python | Migrate, test, collect static files, and deploy |
| Python FastAPI | python | Lint, test, and deploy with health check |
| Go Web Service | go | Format, vet, test, cross-compile, and deploy |
| Static Site | deployment | Build, optimize, and deploy static assets |
| Tauri Desktop | rust | Frontend build, Tauri bundle, and code signing |
| Docker Compose Deploy | docker | Build, push, deploy via docker-compose over SSH |

### Stage Snippet Templates (10)

| Template | Category | Description |
| -------- | -------- | ----------- |
| GitHub Release | deployment | Create a GitHub release with assets |
| Docker Build & Push | docker | Build and push a Docker image to a registry |
| Docker Compose SSH | docker | Deploy via docker-compose over SSH with health check |
| SSH Rsync Deploy | deployment | Deploy files to a remote server via rsync |
| Cargo Publish | rust | Publish a Rust crate to crates.io |
| npm Publish | node | Publish a package to the npm registry |
| S3 Deploy | deployment | Sync build output to an S3 bucket with CloudFront invalidation |
| Tauri Bundle | rust | Bundle a Tauri desktop app for distribution |
| Version Bump & Tag | deployment | Bump semver (patch/minor/major), commit, tag, and push |
| Homebrew Formula | deployment | Update a Homebrew formula with new version and checksum |

## Template Variables

Templates can include `{{variable}}` placeholders that are filled in when the template is applied. When you select a template, Chibby extracts all placeholders and shows a form to fill them in.

### Common Variables

| Variable | Description | Default |
| -------- | ----------- | ------- |
| `project_name` | Name of the project | Auto-detected from repo path |
| `bump_level` | Version bump level (`patch`, `minor`, or `major`) | `patch` |
| `ssh_user` | SSH username for remote deployment | — |
| `ssh_host` | SSH hostname for remote deployment | — |
| `deploy_path` | Remote path for deployment | — |
| `image_name` | Docker image name | — |
| `registry` | Docker registry URL | — |
| `s3_bucket` | AWS S3 bucket name | — |

Variables with defaults are pre-filled. Required variables must be provided before the template can be applied.

### Variable Syntax

Use double braces in any field: stage names, commands, working directories, or health check commands.

```toml
[[stages]]
name = "deploy-{{env}}"
commands = ["rsync -avz ./dist/ {{ssh_user}}@{{ssh_host}}:{{deploy_path}}/"]
```

## Using Templates

### From the Templates Page

1. Navigate to **Templates** in the sidebar
2. Browse or search for a template
3. Expand a template to see its stages
4. Click **Apply Template** to create a new project from it, or **Use as Starting Point** to customize it
5. You are taken to the Add Project wizard where you select a repository, fill in variables, and configure stages

### From the Add Project Wizard

1. In Step 2 (Choose Source), select **From Template**
2. Browse and select a template
3. Fill in any template variables
4. Continue to configure and review stages

### From the Pipeline Editor

1. Click **Stage Templates** in the pipeline settings toolbar
2. Browse stage snippet templates
3. Select a template and fill in variables
4. The stage is appended to your current pipeline

## Creating Custom Templates

### Save from Pipeline Editor

1. Build or edit a pipeline in the Project Detail view
2. Click **Save as Template**
3. Fill in the metadata:
   - **Name** (required) — a unique name for the template
   - **Description** — what the template does
   - **Category** — language or domain (e.g., `rust`, `deployment`)
   - **Tags** — comma-separated tags for filtering
4. Choose scope:
   - **User** — saved to `~/.chibby/templates/`, available globally
   - **Project** — saved to `<repo>/.chibby/templates/`, shareable via git

### Write TOML Manually

Create a `.toml` file in your templates directory with this format:

```toml
[meta]
name = "My Custom Template"
description = "What this template does"
author = "your-name"
version = "1.0.0"
category = "deployment"
tags = ["custom", "deploy"]
required_tools = ["rsync", "ssh"]
template_type = "pipeline"  # or "stage"

# For full pipeline templates:
[pipeline]
name = "{{project_name}} Pipeline"

[[pipeline.stages]]
name = "build"
commands = ["npm run build"]
backend = "local"
fail_fast = true

[[pipeline.stages]]
name = "deploy"
commands = ["rsync -avz ./dist/ {{ssh_host}}:{{deploy_path}}/"]
backend = "local"
fail_fast = true
```

For stage snippet templates, use `[[stages]]` instead of `[pipeline]`:

```toml
[meta]
name = "My Deploy Stage"
description = "Deploy via rsync"
template_type = "stage"
# ... other meta fields

[[stages]]
name = "deploy"
commands = ["rsync -avz ./dist/ {{ssh_host}}:{{deploy_path}}/"]
backend = "local"
fail_fast = true
```

## Import and Export

### Export

From the **Templates** page, click any template's export action to view it as TOML. Copy the output to share with others or save to a file.

### Import

1. Click **Import** on the Templates page
2. Paste the TOML content
3. Choose scope (User or Project)
4. Click **Import**

The imported template appears immediately in the template browser.

## Template Storage

| Scope | Path | Shareable |
| ----- | ---- | --------- |
| Built-in | Embedded in the app binary | N/A |
| User | `~/.chibby/templates/` | No (personal) |
| Project | `<repo>/.chibby/templates/` | Yes (commit to git) |

Templates are stored as individual `.toml` files. The filename is derived from the template name (lowercased, spaces replaced with hyphens).
