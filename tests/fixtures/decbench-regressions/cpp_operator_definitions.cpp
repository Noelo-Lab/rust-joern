struct type_info {
    int value;
    bool operator == (const type_info &other) const;
    bool operator != (const type_info &other) const;
    type_info &operator = (const type_info &other);
};

bool type_info::operator==(const type_info &other) const
{
    return value == other.value;
}

bool type_info::operator!=(const type_info &other) const
{
    return value != other.value;
}

type_info &type_info::operator=(const type_info &other)
{
    value = other.value;
    return *this;
}
