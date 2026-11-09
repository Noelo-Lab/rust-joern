typedef int T; struct S { int a; }; enum E { E1 };
namespace ns { typedef int T; class C {}; namespace inner { struct D {}; } template<class X> struct V2 {}; }
class C2 {}; template<class X> struct V {};
const int &def_reference() { static int x; return x; }
ns::T def_qualified() { return 0; }
ns::T *proto_qualified_pointer();
const ns::T &proto_qualified_reference();
ns::inner::D *proto_nested();
C2 *def_class_pointer() { return 0; }
C2 &proto_class_reference();
auto def_auto() { return 1; }
unsigned long proto_unsigned_long();
struct S *def_struct_pointer() { return 0; }
enum E def_enum() { return E1; }
V<int> *def_template_pointer() { return 0; }
V<int> *proto_template_pointer();
ns::V2<T> &proto_qualified_template();
template<class X> X def_template(X x) { return x; }
template<class X> struct V<X> def_elaborated_template(X x) { V<X> v; return v; }
