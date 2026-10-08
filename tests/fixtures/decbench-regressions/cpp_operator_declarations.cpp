struct type_info {
    bool operator == (const type_info &other) const;
    bool operator != (const type_info &other) const;
    type_info &operator = (const type_info &other);
};
