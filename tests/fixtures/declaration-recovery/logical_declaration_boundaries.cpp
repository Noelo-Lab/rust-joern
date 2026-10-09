typedef int Scalar;
int work(int value);
int global_head;

int logical_unknown(int rhs) { loose_head && rhs == 2; return rhs; }
int logical_unknown_bare(int rhs) { loose_head && rhs; return rhs; }
int logical_unknown_both(int rhs) { loose_head && loose_tail == 2; return rhs; }
int logical_parameter(int head, int rhs) { head && rhs == 2; return rhs; }
int logical_global(int rhs) { global_head && rhs == 2; return rhs; }
int logical_calls(int rhs) { loose_head && work(rhs); return rhs; }
int bitwise_unknown(int rhs) { loose_head & rhs == 2; return rhs; }
int bitwise_parameter(int head, int rhs) { head & rhs == 2; return rhs; }
int multiplication_unknown(int rhs) { loose_head * rhs == 2; return rhs; }
int multiplication_parameter(int head, int rhs) { head * rhs == 2; return rhs; }
int pointer_known(int rhs) { Scalar *pointer = &rhs; return work(*pointer); }
int pointer_unknown(int rhs) { Mystery *pointer = &rhs; return work(*pointer); }
int pointer_unknown_uninitialized(int rhs) { Mystery *pointer; return rhs; }
int lvalue_reference_known(int rhs) { Scalar &reference = rhs; return work(reference); }
int lvalue_reference_unknown(int rhs) { Mystery &reference = rhs; return work(reference); }
int rvalue_reference_known(int rhs) { Scalar &&reference = rhs; return work(reference); }
int rvalue_reference_unknown(int rhs) { Mystery &&reference = rhs; return work(reference); }
int reference_to_pointer(int rhs) { Scalar *pointer = &rhs; Scalar *&reference = pointer; return work(*reference); }
int logical_after_pointer(int rhs) { Mystery *pointer; loose_head && rhs == 2; return rhs; }
int logical_shadowed_alias(int Scalar, int rhs) { Scalar && rhs == 2; return rhs; }
