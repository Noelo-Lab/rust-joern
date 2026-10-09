struct Node { int field; int other; int third; };
int known(int);

int arrow_member_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    p->other;
    known(99);
    return result;
}

int arrow_member_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    p->other;
    known(99);
    return result;
}

int arrow_member_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field
    p->other;
    known(99);
    return result;
}

int arrow_member_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field;
    p->other;
    known(99);
    return result;
}

int arrow_member_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field
        p->other;
    }
    known(99);
    return result;
}

int arrow_member_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field;
        p->other;
    }
    known(99);
    return result;
}

int arrow_member_while_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field
    p->other;
    known(99);
    return result;
}

int arrow_member_while_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field;
    p->other;
    known(99);
    return result;
}

int arrow_member_while_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field
        p->other;
    }
    known(99);
    return result;
}

int arrow_member_while_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field;
        p->other;
    }
    known(99);
    return result;
}

int arrow_member_label_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field
    p->other;
    known(99);
    return result;
}

int arrow_member_label_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field;
    p->other;
    known(99);
    return result;
}

int arrow_declaration_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field;
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field
        int value = known(7);
    }
    known(99);
    return result;
}

int arrow_declaration_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field;
        int value = known(7);
    }
    known(99);
    return result;
}

int arrow_declaration_while_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_while_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field;
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_while_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field
        int value = known(7);
    }
    known(99);
    return result;
}

int arrow_declaration_while_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field;
        int value = known(7);
    }
    known(99);
    return result;
}

int arrow_declaration_label_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field
    int value = known(7);
    known(99);
    return result;
}

int arrow_declaration_label_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field;
    int value = known(7);
    known(99);
    return result;
}

int arrow_return_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int arrow_return_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field;
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int arrow_return_while_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_while_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c)
        p->field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_while_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int arrow_return_while_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    while (c) {
        p->field;
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int arrow_return_label_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int arrow_return_label_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    label_start:
    p->field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int dot_member_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field
    x.other;
    known(99);
    return result;
}

int dot_member_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field;
    x.other;
    known(99);
    return result;
}

int dot_member_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field
    x.other;
    known(99);
    return result;
}

int dot_member_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field;
    x.other;
    known(99);
    return result;
}

int dot_member_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field
        x.other;
    }
    known(99);
    return result;
}

int dot_member_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field;
        x.other;
    }
    known(99);
    return result;
}

int dot_declaration_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field
    int value = known(7);
    known(99);
    return result;
}

int dot_declaration_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field;
    int value = known(7);
    known(99);
    return result;
}

int dot_declaration_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field
    int value = known(7);
    known(99);
    return result;
}

int dot_declaration_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field;
    int value = known(7);
    known(99);
    return result;
}

int dot_declaration_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field
        int value = known(7);
    }
    known(99);
    return result;
}

int dot_declaration_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field;
        int value = known(7);
    }
    known(99);
    return result;
}

int dot_return_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int dot_return_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    x.field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int dot_return_if_scalar_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int dot_return_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        x.field;
    /* tailcall */
    return known(7);
    known(99);
    return result;
}

int dot_return_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int dot_return_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        x.field;
        /* tailcall */
        return known(7);
    }
    known(99);
    return result;
}

int chain_block_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    p->other
    p->third
    known(7);
    known(99);
    return result;
}

int chain_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    p->other;
    p->third;
    known(7);
    known(99);
    return result;
}

int chain_if_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field
        p->other
        p->third
        known(7);
    }
    known(99);
    return result;
}

int chain_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        p->field;
        p->other;
        p->third;
        known(7);
    }
    known(99);
    return result;
}

int chain_else_braced_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) known(1); else {
        p->field
        p->other
        p->third
        known(7);
    }
    known(99);
    return result;
}

int chain_else_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) known(1); else {
        p->field;
        p->other;
        p->third;
        known(7);
    }
    known(99);
    return result;
}

int chain_switch_case_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    switch(c) { case 1:
        p->field
        p->other
        p->third
        known(7);
        break;
        default: break;
    }
    known(99);
    return result;
}

int chain_switch_case_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    switch(c) { case 1:
        p->field;
        p->other;
        p->third;
        known(7);
        break;
        default: break;
    }
    known(99);
    return result;
}

int arrow_assignment_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    result = 7;
    known(99);
    return result;
}

int arrow_assignment_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    result = 7;
    known(99);
    return result;
}

int arrow_index_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    q[1];
    known(99);
    return result;
}

int arrow_index_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    q[1];
    known(99);
    return result;
}

int arrow_call_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    known(7);
    known(99);
    return result;
}

int arrow_call_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    known(7);
    known(99);
    return result;
}

int arrow_identifier_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field
    n;
    known(99);
    return result;
}

int arrow_identifier_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    p->field;
    n;
    known(99);
    return result;
}

int memory_division_block_s(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    *q s/= n;
    known(7);
    known(99);
    return result;
}

int memory_division_block_arbitrary_identifier(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    *q arbitrary_identifier/= n;
    known(7);
    known(99);
    return result;
}

int memory_division_block_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    *q /= n;
    known(7);
    known(99);
    return result;
}

int memory_division_if_scalar_s(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        *q s/= n;
    known(7);
    known(99);
    return result;
}

int memory_division_if_scalar_arbitrary_identifier(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        *q arbitrary_identifier/= n;
    known(7);
    known(99);
    return result;
}

int memory_division_if_scalar_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        *q /= n;
    known(7);
    known(99);
    return result;
}

int memory_division_if_braced_s(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        *q s/= n;
        known(7);
    }
    known(99);
    return result;
}

int memory_division_if_braced_arbitrary_identifier(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        *q arbitrary_identifier/= n;
        known(7);
    }
    known(99);
    return result;
}

int memory_division_if_braced_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c) {
        *q /= n;
        known(7);
    }
    known(99);
    return result;
}

int memory_division_bash_shape(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    *q = n % 0xf4240;
    int saved = *q;
    *q s/= 0x3e8;
    if (saved % 0x3e8 > 0x1f3) *q += 1;
    return *q;
    known(99);
    return result;
}

int scalar_if_member_assignment_missing(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field
    result = 0;
    known(99);
    return result;
}

int scalar_if_member_assignment_valid(int c, struct Node *p, struct Node x, int *q, int n) {
    int result = 0;
    if (c)
        p->field;
    result = 0;
    known(99);
    return result;
}

