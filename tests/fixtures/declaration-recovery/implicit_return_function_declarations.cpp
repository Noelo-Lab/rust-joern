extern extern_proto(int x);
int extern_proto(int x) { return x; }
static static_proto(int x);
int static_proto(int x) { return x; }
inline inline_proto(int x);
int inline_proto(int x) { return x; }
plain_proto(int x);
int plain_proto(int x) { return x; }
extern extern_only(int x);
static static_only(int x);
plain_only(int x);
extern extern_definition(int x) { return x; }
plain_definition(int x) { return x; }
int explicit_control(int x) { return x; }
class Object { public: Object(); Object(int); };
Object::Object() {}
Object::Object(int x) { if(x) x++; }
