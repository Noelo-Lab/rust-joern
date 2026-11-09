namespace demo { template<class T> struct Box { static void touch(int); }; }
void f(int x) { demo::Box<int>::touch(x); }
