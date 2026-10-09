typedef int AliasT;
typedef int AliasU;
void type_use(InferredT a,InferredU b);
int object_both(int T,int U,int x){return (T)(U)x;}
int object_outer(int T,int x){return (T)(UnknownU)x;}
int object_inner(int U,int x){return (UnknownT)(U)x;}
int shadow_outer(int AliasT,int x){return (AliasT)(AliasU)x;}
int shadow_inner(int AliasU,int x){return (AliasT)(AliasU)x;}
int shadow_both(int AliasT,int AliasU,int x){return (AliasT)(AliasU)x;}
int true_typedef_both(int x){return (AliasT)(AliasU)x;}
int unbound_both(int x){return (UnknownT)(UnknownU)x;}
int inferred_both(int x){return (InferredT)(InferredU)x;}
int mixed_signed(int x){signed T var;return (T)(int)x;}
int object_primitive(int T,int x){return (T)(int)x;}
int ordinary_call(int T,int x){return (T)(x);}
int shadow_call(int AliasT,int x){return (AliasT)(x);}
int unknown_call(int x){return (UnknownT)(x);}
int typedef_cast(int x){return (AliasT)(x);}
int primitive_cast(int x){return (int)x;}
int after_control(int x){return x;}
