void target();
void cb();
void ref_single(int x) { if (x) cb(target); }
void ref_multiline(int x) { if (x) cb(
    target
); }
void ref_newline_before(int x) { if (x)
    cb(target);
}
void ref_address(int x) { if (x) cb(
    &target
); }
