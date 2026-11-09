struct Object { ~Object(); int operator~() const; };
Object* next();
void after();
void pointer_call(Object* p) { p->~Object(); after(); }
void reference_call(Object& r) { r.~Object(); after(); }
void receiver_call() { next()->~Object(); after(); }
int complement(int x) { return ~x; }
int operator_call(Object& r) { return r.operator~(); }
