# Security

This is an early-alpha desktop application. It runs as the current user; do not run it with sudo. It does not need system-wide privileges, accept credentials, or upload diagnostics.

Report vulnerabilities through GitHub's private vulnerability-reporting feature when available. Otherwise, contact the maintainer through their GitHub profile before posting exploit details publicly. Never attach Steam configuration backups, credentials, or login tokens to an issue.

Security boundaries:

- Official vendor downloads use certificate-validated HTTPS. Published MD5/SHA-1 metadata checks transfer consistency, not a cryptographic publisher signature.
- Archive paths and symlinks are checked, extraction/download sizes are capped, and files are staged before replacement.
- Updates are serialized and blocked while ETS2 or the official launcher is running.
- Failure to download or validate a mod stops the launch.
- Steam is closed normally before changing its configuration, with an owner-only backup. Existing game saves are not deleted.
- The app cannot defend against a malicious process already running as the same user or a compromised official vendor endpoint.

Only the latest published release receives fixes. Do not add automatic credential capture, disabled certificate verification, third-party executable mirrors, anti-cheat bypasses, or remote shell installation scripts.
