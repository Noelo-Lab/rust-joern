typedef int Scalar;
int work(int value);
int global_head;

int typed_logical_bound(int rhs) { Scalar && rhs; return rhs; }
int typed_logical_fresh(int rhs) { Scalar && fresh; return rhs; }
int unknown_logical_bound(int rhs) { Other && rhs; return rhs; }
int unknown_logical_fresh(int rhs) { Other && fresh; return rhs; }
int inferred_logical_bound(int rhs) { Other *pointer; Other && rhs; return rhs; }
int unknown_logical_comparison(int rhs) { Other && fresh == 2; return rhs; }
int typed_bitwise_bound(int rhs) { Scalar & rhs; return rhs; }
int typed_bitwise_fresh(int rhs) { Scalar & fresh; return rhs; }
int unknown_bitwise_bound(int rhs) { Other & rhs; return rhs; }
int unknown_bitwise_fresh(int rhs) { Other & fresh; return rhs; }
int typed_pointer_bound(int rhs) { Scalar *rhs; return rhs; }
int unknown_pointer_bound(int rhs) { Other *rhs; return rhs; }
int typed_pointer_comparison(int rhs) { Scalar * rhs == 2; return rhs; }
int unknown_pointer_comparison(int rhs) { Other * rhs == 2; return rhs; }
int value_call_rhs(int rhs) { Other && work(rhs); return rhs; }
int reference_prototype(int rhs) { Scalar &&make_reference(int value); return rhs; }
int unknown_reference_prototype(int rhs) { Other &&make_reference(int value); return rhs; }
int pointer_prototype(int rhs) { Scalar *make_pointer(int value); return rhs; }
int lvalue_prototype(int rhs) { Scalar &make_lvalue(int value); return rhs; }
int pointer_and_unknown_comparison(int rhs) { Other *pointer; Other *rhs == 2; return rhs; }
