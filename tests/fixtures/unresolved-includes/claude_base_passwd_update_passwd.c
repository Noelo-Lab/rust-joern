// Function: read_shadow @ 0x352b
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <shadow.h>

/* Node wrapping one shadow entry plus bookkeeping/link fields. */
struct spwd_node {
    struct spwd sp;        /* 0x00: copied shadow entry (72 bytes)      */
    int         active;    /* 0x48                                      */
    void       *key;       /* 0x50: == sp.sp_namp                       */
    int         dup;       /* 0x58                                      */
    void       *link1;     /* 0x60                                      */
    void       *link2;     /* 0x68                                      */
    void       *link3;     /* 0x70                                      */
};

extern int verbosity;

/* Internal helpers (defined elsewhere in the program). */
extern struct spwd_node *spwd_node_new(void);
extern void spwd_node_copy(struct spwd_node *dst, struct spwd *src);
extern void spwd_list_add(void *list, struct spwd_node *node, int flags);

int read_shadow(void *list, const char *filename)
{
    FILE *fp;
    struct spwd *ent;
    struct spwd_node *node;

    if (verbosity > 2)
        printf("Reading shadow from %s\n", filename);

    fp = fopen(filename, "r");
    if (fp == NULL) {
        if (errno != ENOENT)
            fprintf(stderr, "Error opening shadow file %s: %s\n",
                    filename, strerror(errno));
        return 1;
    }

    while ((ent = fgetspent(fp)) != NULL) {
        node = spwd_node_new();
        spwd_node_copy(node, ent);
        node->active = 1;
        node->dup = 0;
        node->key = node->sp.sp_namp;
        if (node->key == NULL)
            break;
        spwd_list_add(list, node, 0);
    }

    if (ent == NULL && errno != ENOENT) {
        fprintf(stderr, "Error reading shadow file %s: %s\n",
                filename, strerror(errno));
        return 2;
    }

    fclose(fp);
    return 0;
}


