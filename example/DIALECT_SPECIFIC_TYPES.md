# Dialect-Specific Column Types

This document explains how to use database-specific column types in Stratum schema definitions.

## Overview

Stratum supports two approaches to defining column types:

1. **Common Types**: Work across all databases (PostgreSQL, MySQL, SQLite)
2. **Dialect-Specific Types**: Leverage database-specific features

## When to Use Dialect-Specific Types

Use dialect-specific types when:
- You need database-specific features (e.g., PostgreSQL SERIAL, MySQL ENUM)
- You want to optimize for a specific database
- You're migrating existing schemas that use specific types

Use common types when:
- You want database portability
- The common type meets your requirements
- You might switch databases in the future

## PostgreSQL-Specific Types

### Auto-Incrementing Types

```yaml
# SERIAL - Auto-incrementing integer (32-bit)
- name: id
  type:
    kind: SERIAL
  nullable: false

# BIGSERIAL - Auto-incrementing big integer (64-bit)
- name: id
  type:
    kind: BIGSERIAL
  nullable: false

# SMALLSERIAL - Auto-incrementing small integer (16-bit)
- name: id
  type:
    kind: SMALLSERIAL
  nullable: false
```

### Integer Types with Specific Sizes

```yaml
# INT2 - 2-byte integer (-32768 to 32767)
- name: age
  type:
    kind: INT2
  nullable: false

# INT4 - 4-byte integer (standard integer)
- name: count
  type:
    kind: INT4
  nullable: false

# INT8 - 8-byte integer (big integer)
- name: large_number
  type:
    kind: INT8
  nullable: false
```

### Network Types

```yaml
# INET - IPv4 or IPv6 address
- name: ip_address
  type:
    kind: INET
  nullable: true

# CIDR - Network address with subnet mask
- name: network
  type:
    kind: CIDR
  nullable: true
```

### Bit String Types

```yaml
# VARBIT - Variable-length bit string
- name: flags
  type:
    kind: VARBIT
    length: 16  # Maximum 16 bits
  nullable: true

# Without length (unlimited)
- name: variable_flags
  type:
    kind: VARBIT
  nullable: true
```

### Array Types

```yaml
# ARRAY - Array of elements
- name: tags
  type:
    kind: ARRAY
    element_type: TEXT
  nullable: true

- name: scores
  type:
    kind: ARRAY
    element_type: INTEGER
  nullable: true
```

## MySQL-Specific Types

### Integer Types

```yaml
# TINYINT - Very small integer
- name: age
  type:
    kind: TINYINT
    unsigned: true  # 0 to 255, default is signed (-128 to 127)
  nullable: false

# MEDIUMINT - Medium-sized integer
- name: population
  type:
    kind: MEDIUMINT
    unsigned: true
  nullable: false
```

### Enumeration Types

```yaml
# ENUM - String with predefined values
- name: status
  type:
    kind: ENUM
    values: ["active", "inactive", "pending"]
  nullable: false

- name: priority
  type:
    kind: ENUM
    values: ["low", "medium", "high", "critical"]
  nullable: false
  default_value: "medium"
```

### Set Types

```yaml
# SET - Set of string values (can have multiple)
- name: permissions
  type:
    kind: SET
    values: ["read", "write", "execute", "delete"]
  nullable: true

- name: features
  type:
    kind: SET
    values: ["email", "sms", "push", "webhook"]
  nullable: false
```

### Year Type

```yaml
# YEAR - Year in 4-digit format
- name: birth_year
  type:
    kind: YEAR
  nullable: true
```

## Complete Example

See [dialect_specific_example.yaml](schema/dialect_specific_example.yaml) for a complete working example.

```yaml
version: "1.0"
tables:
  users:
    name: users
    columns:
      # PostgreSQL SERIAL for auto-incrementing ID
      - name: id
        type:
          kind: SERIAL
        nullable: false

      # Common VARCHAR type (works everywhere)
      - name: username
        type:
          kind: VARCHAR
          length: 50
        nullable: false

      # MySQL ENUM for status
      - name: status
        type:
          kind: ENUM
          values: ["active", "inactive", "banned"]
        nullable: false
        default_value: "active"

      # PostgreSQL INET for IP addresses
      - name: last_ip
        type:
          kind: INET
        nullable: true

      # Common TIMESTAMP (works everywhere)
      - name: created_at
        type:
          kind: TIMESTAMP
          with_time_zone: true
        nullable: false

    constraints:
      - type: PRIMARY_KEY
        columns:
          - id

    indexes:
      - name: idx_users_username
        columns:
          - username
        unique: true
```

## Error Handling

Dialect-specific types are validated by the database at migration time. If you use an invalid type:

```yaml
# Typo: "SERIALS" instead of "SERIAL"
- name: id
  type:
    kind: SERIALS  # ❌ This will cause a database error
  nullable: false
```

You'll receive a clear error message from the database:

```
ERROR:  type "SERIALS" does not exist
LINE 1: CREATE TABLE users (id SERIALS);
                               ^
HINT:  Did you mean "SERIAL"?
```

## IDE Support

For the best development experience with auto-completion:

### VSCode

1. Install the [YAML extension by Red Hat](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml)
2. The schema is already configured in `.vscode/settings.json`
3. Start typing in your YAML files to see suggestions

### IntelliJ IDEA / WebStorm

1. Go to Settings → Languages & Frameworks → Schemas and DTDs → JSON Schema Mappings
2. Add mapping:
   - Schema file: `resources/schemas/stratum-schema.json`
   - File pattern: `schema/**/*.yaml`

## Best Practices

1. **Document your choice**: Add comments explaining why you chose a dialect-specific type
2. **Test early**: Run migrations in development to catch type errors early
3. **Be consistent**: If using dialect-specific types, use them throughout the schema
4. **Consider portability**: Mixing common and dialect-specific types can make migration harder

## Migrating from Common to Dialect-Specific Types

To migrate from common types to dialect-specific types:

1. Update the type in your YAML schema
2. Generate a new migration: `stratum generate`
3. Review the generated SQL
4. Test in development before production

Example migration:

```yaml
# Before (common type)
- name: id
  type:
    kind: INTEGER
  nullable: false
  auto_increment: true

# After (PostgreSQL-specific)
- name: id
  type:
    kind: SERIAL
  nullable: false
```

This will generate an `ALTER TABLE` migration to change the column type.
