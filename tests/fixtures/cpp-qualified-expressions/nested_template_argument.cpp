namespace demo { template<class T> struct Box { static int get(int); }; }
int sink(int);
int f(int x) { return sink(demo::Box<int>::get(x)); }
