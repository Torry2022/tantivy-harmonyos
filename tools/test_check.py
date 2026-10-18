import json
from pathlib import Path
import unittest
from check import check_contract


class FrozenContract(unittest.TestCase):
    def setUp(self):
        root = Path(__file__).resolve().parents[1]
        self.header = (root / "include/tantivy_ohos.h").read_text(encoding="utf-8")
        self.frozen = json.loads((root / "tests/abi4-contract.json").read_text())["declarations"]

    def test_published_declarations_survive(self):
        check_contract(self.header, self.frozen)

    def test_return_type_change_fails(self):
        with self.assertRaises(ValueError):
            check_contract(self.header.replace("uint32_t tantivy_ohos_abi_version",
                                               "int tantivy_ohos_abi_version"), self.frozen)

    def test_layout_change_fails(self):
        with self.assertRaises(ValueError):
            check_contract(self.header.replace("float score;", "double score;"), self.frozen)

    def test_removed_function_fails(self):
        with self.assertRaises(ValueError):
            check_contract(self.header.replace("tantivy_ohos_reload(", "removed_reload("), self.frozen)


if __name__ == "__main__":
    unittest.main()
