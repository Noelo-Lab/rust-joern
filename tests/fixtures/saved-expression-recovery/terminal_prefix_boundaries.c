int known();
int bound_empty_missing_compound(int x){known()}
int bound_empty_missing_scalar_if(int x){if(x)known()}
int bound_empty_missing_braced_if(int x){if(x){known()}}
int bound_empty_proper_compound(int x){known();}
int bound_empty_proper_scalar_if(int x){if(x)known();}
int bound_empty_proper_braced_if(int x){if(x){known();}}
int unknown_empty_missing_compound(int x){unknown()}
int unknown_empty_missing_scalar_if(int x){if(x)unknown()}
int unknown_empty_missing_braced_if(int x){if(x){unknown()}}
int unknown_empty_proper_compound(int x){unknown();}
int unknown_empty_proper_scalar_if(int x){if(x)unknown();}
int unknown_empty_proper_braced_if(int x){if(x){unknown();}}
int bound_plain_missing_compound(int x){known(x)}
int bound_plain_missing_scalar_if(int x){if(x)known(x)}
int bound_plain_missing_braced_if(int x){if(x){known(x)}}
int bound_plain_proper_compound(int x){known(x);}
int bound_plain_proper_scalar_if(int x){if(x)known(x);}
int bound_plain_proper_braced_if(int x){if(x){known(x);}}
int bound_compound_missing_compound(int x){known(x!=0)}
int bound_compound_missing_scalar_if(int x){if(x)known(x!=0)}
int bound_compound_missing_braced_if(int x){if(x){known(x!=0)}}
int bound_compound_proper_compound(int x){known(x!=0);}
int bound_compound_proper_scalar_if(int x){if(x)known(x!=0);}
int bound_compound_proper_braced_if(int x){if(x){known(x!=0);}}
int assignment_missing_compound(int x){x=1}
int assignment_missing_scalar_if(int x){if(x)x=1}
int assignment_missing_braced_if(int x){if(x){x=1}}
int assignment_proper_compound(int x){x=1;}
int assignment_proper_scalar_if(int x){if(x)x=1;}
int assignment_proper_braced_if(int x){if(x){x=1;}}
int increment_missing_compound(int x){x++}
int increment_missing_scalar_if(int x){if(x)x++}
int increment_missing_braced_if(int x){if(x){x++}}
int increment_proper_compound(int x){x++;}
int increment_proper_scalar_if(int x){if(x)x++;}
int increment_proper_braced_if(int x){if(x){x++;}}
int return_missing_compound(int x){return x}
int return_missing_scalar_if(int x){if(x)return x}
int return_missing_braced_if(int x){if(x){return x}}
int return_proper_compound(int x){return x;}
int return_proper_scalar_if(int x){if(x)return x;}
int return_proper_braced_if(int x){if(x){return x;}}
int primitive_decl_missing_compound(int x){int y}
int primitive_decl_missing_scalar_if(int x){if(x)int y}
int primitive_decl_missing_braced_if(int x){if(x){int y}}
int primitive_decl_proper_compound(int x){int y;}
int primitive_decl_proper_scalar_if(int x){if(x)int y;}
int primitive_decl_proper_braced_if(int x){if(x){int y;}}
int unknown_decl_missing_compound(int x){UnknownType value}
int unknown_decl_missing_scalar_if(int x){if(x)UnknownType value}
int unknown_decl_missing_braced_if(int x){if(x){UnknownType value}}
int unknown_decl_proper_compound(int x){UnknownType value;}
int unknown_decl_proper_scalar_if(int x){if(x)UnknownType value;}
int unknown_decl_proper_braced_if(int x){if(x){UnknownType value;}}
int after_control(int x){return x;}
