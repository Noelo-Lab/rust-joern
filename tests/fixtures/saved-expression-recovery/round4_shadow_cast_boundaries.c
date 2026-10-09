typedef int T;
int known(int x);
void type_use(Inferred argument);
int object_literal(int Plain,int x){return (Plain)0;}
int object_tilde(int Plain,int x){return (Plain)~x;}
int object_pointer(int Plain,int x){return (Plain *)x;}
int object_multiply(int Plain,int x){return (Plain)*x;}
int object_addition(int Plain,int x){return (Plain)+x;}
int object_parenthesized_call(int Plain,int x){return (Plain)(x);}
int typedef_shadow_literal(int T,int x){return (T)0;}
int typedef_shadow_tilde(int T,int x){return (T)~x;}
int typedef_shadow_pointer(int T,int x){return (T *)x;}
int typedef_shadow_multiply(int T,int x){return (T)*x;}
int typedef_shadow_addition(int T,int x){return (T)+x;}
int typedef_shadow_parenthesized_call(int T,int x){return (T)(x);}
int typedef_literal(int x){return (T)0;}
int typedef_tilde(int x){return (T)~x;}
int typedef_pointer(int x){return (T *)x;}
int typedef_multiply(int x){return (T)*x;}
int typedef_addition(int x){return (T)+x;}
int typedef_parenthesized_call(int x){return (T)(x);}
int unknown_literal(int x){return (UnknownName)0;}
int unknown_tilde(int x){return (UnknownName)~x;}
int unknown_pointer(int x){return (UnknownName *)x;}
int unknown_multiply(int x){return (UnknownName)*x;}
int unknown_addition(int x){return (UnknownName)+x;}
int unknown_parenthesized_call(int x){return (UnknownName)(x);}
int inferred_literal(int x){return (Inferred)0;}
int inferred_tilde(int x){return (Inferred)~x;}
int inferred_pointer(int x){return (Inferred *)x;}
int inferred_multiply(int x){return (Inferred)*x;}
int inferred_addition(int x){return (Inferred)+x;}
int inferred_parenthesized_call(int x){return (Inferred)(x);}
int positive_literal(int x){return (int)0;}
int positive_tilde(int x){return (int)~x;}
int positive_pointer(int x){return (int *)x;}
int positive_cast_deref(int *x){return (int)*x;}
int positive_cast_plus(int x){return (int)+x;}
int positive_parenthesized_callee(int x){return (known)(x);}
int after_control(int x){return x;}
