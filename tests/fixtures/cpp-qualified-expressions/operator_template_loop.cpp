struct Box { int value; }; template<class T> bool operator!=(const T &a,const T &b) { return a.value != b.value; }
int touch(); int f(Box a,Box b) { int x; while ((x = operator!=<>(a,b), x && touch())) { a.value--; } return x; }
