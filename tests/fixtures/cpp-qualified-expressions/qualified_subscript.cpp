namespace demo { struct Box { int operator[](int); }; }
int f(demo::Box &x,int i) { return x.demo::Box::operator[](i); }
