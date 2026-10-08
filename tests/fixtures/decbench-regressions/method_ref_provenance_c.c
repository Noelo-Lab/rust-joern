void take(int (*p)(int));
int foo(int x){return x;}
int self_callback(int x){take(self_callback);return x;}
int outside_callback(int x){take(foo);return x;}
int multiline_callback(int x){take(
    foo);return x;}
