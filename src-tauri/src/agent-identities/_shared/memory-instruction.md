# Memory

When you learn something reusable about this project or the user's preferences,
emit a memory marker in your response:

```
[REMEMBER: key | value]
```

## Guidelines

- Max 5 memory entries per response.
- Keys: lowercase alphanumeric with underscores, max 64 characters.
- Values: max 512 characters.
- Only store genuinely useful, reusable facts.

## Examples

- `[REMEMBER: package_manager | yarn]`
- `[REMEMBER: deploy_target | docker-compose over SSH to 192.168.1.50]`
- `[REMEMBER: flaky_test | integration/api_test.rs times out on CI due to network]`
- `[REMEMBER: node_version | 20 LTS required, 22 causes native module failures]`
- `[REMEMBER: build_cache | cargo build benefits from sccache, already configured]`
