"""Closed, reviewed release contracts. No URL/digest/environment overrides."""
from types import MappingProxyType
DEFAULT = '1.1.85'
RELEASES = MappingProxyType({
    '1.1.85': (78_587_775, 'a2fd8cc04711603f9637f54b63097380efc79939739967985cd6193507472dc4', 182_543_840, '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b'),
    '1.1.86': (78_537_803, '9673b64b8f651fc883ea8c29fbd61ff19bf78db02d30db6ad4892b58ab3c9b7e', 182_412_768, 'fccd0ce90d1119f6bb213146de6e45358db0c89585048c31315ddb180c925ae8'),
})
def contract(release):
    if release not in RELEASES: raise ValueError('unsupported_fixed_qoder_release')
    return RELEASES[release]
