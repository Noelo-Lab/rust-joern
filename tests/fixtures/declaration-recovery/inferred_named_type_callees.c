struct sigaction { int handler; };
int sigaction(int, void *, void *);
int named_type_then_call(int n) { sigaction local; sigaction(n, &local, 0); return n; }
int after_named_type(int n) { sigaction(n, 0, 0); return n; }
int single_identifier_call(int n) { Unknown local; Unknown(n); return n; }
int literal_call(int n) { Unknown local; Unknown(1); return n; }
int unary_argument_call(int n) { Unknown local; Unknown(&n); return n; }
int new_grouped_object(int n) { Unknown local; Unknown(other) = n; return n; }
int after_grouped_object(int n) { Unknown(n, 0); return n; }
