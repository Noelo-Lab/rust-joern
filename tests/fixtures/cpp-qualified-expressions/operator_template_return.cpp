struct Box { int value; }; template<class T> bool operator!=(const T &a,const T &b) { return a.value != b.value; }
int f(Box a,Box b) { return operator!=<>(a,b); }
