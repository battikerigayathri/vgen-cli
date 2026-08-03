# Push error codes

| Code | Exit | Severity | Description | Remediation |
|------|------|----------|-------------|-------------|
| `PUSH_BLOCKED` | 3 | error | push-all preflight found validation blockers | Run `resmate validate`; fix blockers or use `--force` (risky) |
| `PUSH_STEP_FAILED` | 1 | error | A push-all step failed during execution | Check API response; fix artifact; re-run `push-all --dry-run` |
| `PUSH_PARTIAL` | 1 | error | push-all stopped mid-run after partial success | Review `rollback_journal` in JSON output; manually reconcile platform state |
