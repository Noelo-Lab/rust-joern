struct Object { ~Object(); };
void spaced(Object* p) { p->~ Object(); }
void alternate(Object* p) { p->compl Object(); }
void qualified(Object* p) { p->Object::compl Object(); }
