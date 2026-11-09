namespace demo { template<class T> struct Box { static int get(int); }; }
int f(int x) { return demo::Box<int>::get(x); }
