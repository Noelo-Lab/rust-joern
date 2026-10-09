// Function: main @ 0x2620
/* r2dec pseudo code output (r2 6.2.0) */
/* /in/sulogin @ 0x2620 */
#include <stdint.h>
 
int32_t main (int32_t argc, char ** argv) {
    int32_t var_ch;
    int64_t canary;
    rdi = argc;
    rsi = argv;
    /* [16] -r-x section size 3827 named .text */
    r12d = edi;
    edi = 0;
    rbx = *(obj._environ);
    rax = *(fs:0x28);
    *((rsp + 0x48)) = rax;
    eax = 0;
    r13 = rsp;
    rsi = rsp;
    tcgetattr ();
    rdx = r13;
    esi = 0;
    edi = 0;
    r13 = 0x00004261;
    rax = 0x8000000500;
    tcsetattr ();
    rax = fcn_00002ac0 (*(rbp));
    rdi = rax;
    *(0x00006060) = rax;
    fcn_000034a0 (rdi);
    fcn_000034c0 (*(obj.stderr));
    setlocale (6, 0x00004152);
    bindtextdomain (r13, "/usr/share/locale");
    textdomain (r13, rsi);
    fcn_00002ba0 ();
    if (r12d > 1) {
        close (0);
        close (1);
        eax = close (2);
        eax = 0;
        eax = open (*((rbp + 8)), 2, rdx);
        if (eax < 0) {
            goto label_1;
        }
        edi = 0;
        dup ();
        edi = 0;
        dup ();
    }
    eax = access ("/etc/passwd", 0);
    if (eax == 0xffffffff) {
        goto label_2;
    }
    eax = isatty (0);
    if (eax == 0) {
        goto label_1;
    }
    eax = isatty (1);
    if (eax == 0) {
        goto label_1;
    }
    eax = isatty (2);
    if (eax == 0) {
        goto label_1;
    }
    eax = getppid ();
    if (eax != 1) {
        goto label_0;
    }
    goto label_3;
    do {
        rbx += 8;
        fcn_00002bd0 (rdi, 0);
label_0:
        rdi = *(rbx);
    } while (rdi != 0);
    rsi = 0x00002aa0;
    *(0x000080c0) = 0x746f6f72;
    rbx = 0x00006080;
    *(0x000080c4) = 0;
    r12 = 0x000080c0;
    r13 = "\nType control-d to proceed with normal startup,\n(or give root password for system maintenance):";
    signal (0xe);
    edi = 0x3c;
    rbp = 0x000060c0;
    r14 = "Login incorrect";
    alarm ();
    while (al == 0) {
        sleep (2);
        edx = 5;
        rax = dcgettext (0, r14);
        puts (rax);
        fcn_00002af0 (r12, rbx);
        edx = 5;
        if (*(rbx) == 0) {
            goto label_4;
        }
        rax = dcgettext (0, r13);
        rdi = rax;
        rax = getpass ();
        r15 = rax;
        if (rax == 0) {
            goto label_5;
        }
        if (*(rax) == 0) {
            goto label_5;
        }
        strncpy (rbp, rax, 0x1fff);
        *(0x000080bf) = 0;
        rax = strlen (r15);
        rdi = r15;
        rdx = 0xffffffffffffffff;
        rsi = rax;
        explicit_bzero_chk ();
        al = fcn_000032b0 (rbp, rbx);
    }
    edx = segment.LOAD1;
    esi = segment.LOAD1;
    rdi = rbp;
    explicit_bzero_chk ();
    edi = 0;
    alarm ();
    esi = 0;
    signal (0xe);
    rax = *(0x0000a0c0);
    edx = 5;
    *(obj._environ) = rax;
    rax = dcgettext (0, "Entering System Maintenance Mode");
    puts (rax);
    eax = fcn_00003140 (*(0x000060a8), 0, *(obj._environ));
    al = (eax == 2) ? 1 : 0;
    eax = (int32_t) al;
    eax += 0x7e;
    rdx = *((rsp + 0x48));
    rdx -= *(fs:0x28);
    if (rdx != 0) {
        goto label_6;
    }
    return rax;
label_3:
    eax = setsid ();
    eax = 0;
    eax = ioctl (0, 0x540e, 1);
    if (eax == 0) {
        goto label_0;
    }
    rbp = *(obj.stderr);
    edx = 5;
    rax = dcgettext (0, "TIOCSCTTY failed");
    fputs (rax, rbp);
    goto label_0;
label_5:
    puts (0x00004152);
    exit (0);
label_4:
    do {
        rax = dcgettext (0, "No password entry for 'root');
        puts (rax);
label_1:
        exit (1);
label_6:
        stack_chk_fail ();
label_2:
        edx = 5;
        rsi = "No password file";
    } while (1);
}

