The unmodified read-config-file.ts is from QwenLM/qwen-code commit
6c3d5355de2be05b8d1ab61f4699f52c02c42068, Git blob
0c6efec151e2576a005d3c9c63b47f6418c184e4 (Apache-2.0; license included).
https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/src/config/read-config-file.ts

This small offline fixture is used for the upstream bounded file-read contract
only. The script also uses JSON.parse, the upstream strict-JSON parse operation.
It does NOT import the complete settings loader, resolve multiple scopes, run
Qwen, prove effective settings or verify package authenticity.

Release compatibility review: this exact reader (SHA-256
64c62594e00b4f5c5acfddfdf8c92ff9f6e057c32c36f3e3a8f30742fbbb279c)
is also byte-identical at the npm release gitHeads:
- 0.27.0: 03b0c2435be1890a21b117860eaf389076787275
- 0.27.2: f42fe0f3251172c1174d00382ece522eafb3c6c2
The original 6c3d5355 attribution is not a claim that it is the published gitHead.
See ../qwen-reviewed-releases/evidence.json and the static CI verifier.
