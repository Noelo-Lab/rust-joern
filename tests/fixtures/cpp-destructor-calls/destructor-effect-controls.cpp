struct Object { ~Object(); };
bool guard();
Object* next();
Object* objects();
int index();
void after();
void conditional() { guard() && (next()->~Object(), true); after(); }
void indexed() { objects()[index()].~Object(); after(); }
template<class T> struct Box { ~Box(); };
Box<int>* next_box();
void qualified() { next_box()->Box<int>::~Box(); after(); }
