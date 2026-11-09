struct Object { ~Object(); };
int tick();
void after();
void f(Object* p) { p->~Object(tick()); after(); }
