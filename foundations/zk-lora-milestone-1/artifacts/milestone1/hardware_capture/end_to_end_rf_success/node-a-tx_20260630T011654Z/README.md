# node-a-tx End-to-End RF TX Attempt

Captured UTC: Tue Jun 30 01:16:59 UTC 2026

Purpose:
A-side TX preparation for end-to-end RF success evidence.

Result:
Concentrator recovery did not produce CONCENTRATOR_RECOVERY_PASS=YES, so A did not transmit. This preserves failure evidence honestly.

Included:
- deterministic payload path/stat/SHA256 if available
- concentrator recovery log
- recovery exit code

Evidence rule:
No end-to-end RF success is claimed from this artifact.
