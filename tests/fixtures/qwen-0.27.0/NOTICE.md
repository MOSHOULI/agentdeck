The unmodified read-config-file.ts is from QwenLM/qwen-code commit
6c3d5355de2be05b8d1ab61f4699f52c02c42068, Git blob
0c6efec151e2576a005d3c9c63b47f6418c184e4 (Apache-2.0; license included).
https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/src/config/read-config-file.ts

This small offline fixture is used for the upstream bounded file-read contract
only. The script also uses JSON.parse, the upstream strict-JSON parse operation.
It does NOT import the complete settings loader, resolve multiple scopes, run
Qwen, prove effective settings or verify package authenticity.
