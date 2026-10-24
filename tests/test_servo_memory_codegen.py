"""Execute the vendored DOM vtable generator without loading the WebIDL parser."""
import ast
from pathlib import Path
from types import SimpleNamespace
import unittest


class MemoryVtableTests(unittest.TestCase):
    def test_box_and_rc_interfaces_use_their_actual_allocation_kind(self):
        source = Path(__file__).resolve().parents[1] / 'vendor/servo-script-bindings/codegen/codegen.py'
        tree = ast.parse(source.read_text())
        function = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'DOMClass')
        namespace = {'Descriptor': object, 'DOMClassTypeId': lambda _: 'FixtureType'}
        exec(compile(ast.Module(body=[function], type_ignores=[]), str(source), 'exec'), namespace)
        for is_rc in [False, True]:
            with self.subTest(is_rc=is_rc):
                descriptor = SimpleNamespace(prototypeChain=['Fixture'], config=SimpleNamespace(maxProtoChainLength=1),
                                             concreteType='Fixture', weakReferenceable=is_rc, prototypeDepth=0,
                                             isGlobal=lambda: False)
                generated = namespace['DOMClass'](descriptor)
                self.assertIn(f'malloc_size_of_including_raw_self::<Fixture, {str(is_rc).lower()}>', generated)
                self.assertIn('as unsafe fn(&mut _, _) -> _', generated)


if __name__ == '__main__':
    unittest.main()
