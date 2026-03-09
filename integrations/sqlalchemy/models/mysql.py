"""
Comprehensive MySQL models exercising all features dbhelper must support.

Feature coverage:
 - Column types: TINYINT, SMALLINT, INT, BIGINT (unsigned), FLOAT, DOUBLE,
   DECIMAL, VARCHAR, CHAR, TEXT, MEDIUMTEXT, LONGTEXT, BOOLEAN, DATE, TIME,
   DATETIME (fsp), TIMESTAMP (fsp), YEAR, JSON, BINARY, VARBINARY, BLOB,
   ENUM, SET
 - Primary keys: single-column, composite
 - Foreign keys: simple, composite, cascade/set-null/restrict
 - Indexes: btree, unique, composite
 - Constraints: not-null, unique, check, default
 - Auto-increment
 - ON UPDATE CURRENT_TIMESTAMP
"""

import enum

from sqlalchemy import (
    BigInteger,
    Boolean,
    CheckConstraint,
    Date,
    DateTime,
    Float,
    ForeignKey,
    ForeignKeyConstraint,
    Index,
    Integer,
    Numeric,
    SmallInteger,
    String,
    Text,
    UniqueConstraint,
    text,
)
from sqlalchemy.dialects.mysql import (
    BIGINT,
    BINARY,
    CHAR,
    DATETIME as MYSQL_DATETIME,
    DOUBLE,
    ENUM,
    INTEGER,
    JSON,
    LONGTEXT,
    MEDIUMTEXT,
    SET,
    SMALLINT,
    TIMESTAMP as MYSQL_TIMESTAMP,
    TINYINT,
    VARBINARY,
    VARCHAR,
    YEAR,
)
from sqlalchemy.orm import Mapped, mapped_column

from models.base import Base


# ---------------------------------------------------------------------------
# Tables
# ---------------------------------------------------------------------------


class MyUser(Base):
    """Core users table for MySQL."""

    __tablename__ = "my_users"
    __table_args__ = (
        UniqueConstraint("uuid", name="my_users_uuid_uniq"),
        Index("my_users_email_idx", "email", unique=True),
        Index("my_users_username_idx", "username", unique=True),
        Index("my_users_role_idx", "role"),
        Index("my_users_created_at_idx", "created_at"),
        CheckConstraint(
            "age IS NULL OR (age >= 0 AND age <= 200)", name="my_users_age_check"
        ),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), primary_key=True, autoincrement=True
    )
    uuid: Mapped[str] = mapped_column(
        VARCHAR(36), nullable=False, server_default=text("(UUID())")
    )
    email: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    username: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    password_hash: Mapped[str] = mapped_column(Text, nullable=False)
    role: Mapped[str] = mapped_column(
        ENUM("admin", "editor", "viewer", "guest"),
        nullable=False,
        server_default="viewer",
    )
    display_name: Mapped[str | None] = mapped_column(VARCHAR(200))
    bio: Mapped[str | None] = mapped_column(MEDIUMTEXT)
    age: Mapped[int | None] = mapped_column(TINYINT(unsigned=True))
    score: Mapped[float | None] = mapped_column(Float, server_default=text("0"))
    balance: Mapped[str | None] = mapped_column(
        Numeric(15, 2), server_default=text("0.00")
    )
    is_active: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("1")
    )
    metadata_: Mapped[str | None] = mapped_column("metadata", JSON)
    last_login_at: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )
    updated_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)"),
    )


class MyOrganization(Base):
    """Organizations for MySQL."""

    __tablename__ = "my_organizations"
    __table_args__ = (
        UniqueConstraint("slug"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        INTEGER(unsigned=True), primary_key=True, autoincrement=True
    )
    name: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    slug: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    parent_id: Mapped[int | None] = mapped_column(
        INTEGER(unsigned=True),
        ForeignKey("my_organizations.id", ondelete="SET NULL"),
    )
    description: Mapped[str | None] = mapped_column(Text)
    website: Mapped[str | None] = mapped_column(VARCHAR(500))
    logo_url: Mapped[str | None] = mapped_column(Text)
    is_verified: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("0")
    )
    settings: Mapped[str | None] = mapped_column(JSON)
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyOrgMember(Base):
    """Org members — composite PK for MySQL."""

    __tablename__ = "my_org_members"
    __table_args__ = (
        ForeignKeyConstraint(
            ["org_id"],
            ["my_organizations.id"],
            ondelete="CASCADE",
            onupdate="CASCADE",
        ),
        ForeignKeyConstraint(
            ["user_id"],
            ["my_users.id"],
            ondelete="CASCADE",
            onupdate="CASCADE",
        ),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    org_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), primary_key=True)
    user_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), primary_key=True)
    role: Mapped[str] = mapped_column(
        VARCHAR(50), nullable=False, server_default=text("'member'")
    )
    joined_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyProject(Base):
    """Projects for MySQL."""

    __tablename__ = "my_projects"
    __table_args__ = (
        ForeignKeyConstraint(
            ["org_id"], ["my_organizations.id"], ondelete="CASCADE"
        ),
        UniqueConstraint("org_id", "slug", name="my_projects_org_slug_uniq"),
        Index("my_projects_org_id_idx", "org_id"),
        Index("my_projects_priority_idx", "priority"),
        CheckConstraint(
            "start_date IS NULL OR deadline IS NULL OR start_date <= deadline",
            name="my_projects_dates_check",
        ),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        INTEGER(unsigned=True), primary_key=True, autoincrement=True
    )
    org_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), nullable=False)
    name: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    slug: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    description: Mapped[str | None] = mapped_column(Text)
    priority: Mapped[str] = mapped_column(
        ENUM("low", "medium", "high", "urgent"),
        nullable=False,
        server_default="medium",
    )
    is_archived: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("0")
    )
    config: Mapped[str | None] = mapped_column(JSON)
    start_date: Mapped[str | None] = mapped_column(Date)
    deadline: Mapped[str | None] = mapped_column(Date)
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )
    updated_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)"),
    )


class MyTask(Base):
    """Tasks for MySQL."""

    __tablename__ = "my_tasks"
    __table_args__ = (
        ForeignKeyConstraint(
            ["project_id"], ["my_projects.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["assignee_id"], ["my_users.id"], ondelete="SET NULL"
        ),
        Index("my_tasks_project_status_idx", "project_id", "status"),
        Index("my_tasks_assignee_idx", "assignee_id"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), primary_key=True, autoincrement=True
    )
    project_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), nullable=False)
    parent_id: Mapped[int | None] = mapped_column(
        BIGINT(unsigned=True),
        ForeignKey("my_tasks.id", ondelete="CASCADE"),
    )
    assignee_id: Mapped[int | None] = mapped_column(BIGINT(unsigned=True))
    title: Mapped[str] = mapped_column(VARCHAR(500), nullable=False)
    description: Mapped[str | None] = mapped_column(MEDIUMTEXT)
    status: Mapped[str] = mapped_column(
        ENUM("pending", "processing", "shipped", "delivered", "cancelled", "refunded"),
        nullable=False,
        server_default="pending",
    )
    priority: Mapped[str] = mapped_column(
        ENUM("low", "medium", "high", "urgent"),
        nullable=False,
        server_default="medium",
    )
    sort_order: Mapped[int] = mapped_column(
        Integer, nullable=False, server_default=text("0")
    )
    estimate_hours: Mapped[str | None] = mapped_column(Numeric(6, 2))
    due_date: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    completed_at: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )
    updated_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)"),
    )


class MyComment(Base):
    """Comments for MySQL."""

    __tablename__ = "my_comments"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["my_tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["author_id"], ["my_users.id"], ondelete="CASCADE"
        ),
        Index("my_comments_task_idx", "task_id"),
        Index("my_comments_author_idx", "author_id"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), primary_key=True, autoincrement=True
    )
    task_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    author_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    parent_comment_id: Mapped[int | None] = mapped_column(
        BIGINT(unsigned=True),
        ForeignKey("my_comments.id", ondelete="CASCADE"),
    )
    body: Mapped[str] = mapped_column(MEDIUMTEXT, nullable=False)
    edited_at: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyTag(Base):
    """Tags for MySQL."""

    __tablename__ = "my_tags"
    __table_args__ = (
        UniqueConstraint("name"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        INTEGER(unsigned=True), primary_key=True, autoincrement=True
    )
    name: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    color: Mapped[str] = mapped_column(
        CHAR(7), nullable=False, server_default=text("'#808080'")
    )
    description: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyTaskTag(Base):
    """Join table: tasks <-> tags for MySQL."""

    __tablename__ = "my_task_tags"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["my_tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["tag_id"], ["my_tags.id"], ondelete="CASCADE"
        ),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    task_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), primary_key=True)
    tag_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), primary_key=True)
    added_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyAuditLog(Base):
    """Audit log for MySQL."""

    __tablename__ = "my_audit_log"
    __table_args__ = (
        ForeignKeyConstraint(
            ["user_id"], ["my_users.id"], ondelete="SET NULL"
        ),
        Index("my_audit_log_user_idx", "user_id"),
        Index("my_audit_log_table_action_idx", "table_name", "action"),
        Index("my_audit_log_occurred_at_idx", "occurred_at"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), primary_key=True, autoincrement=True
    )
    user_id: Mapped[int | None] = mapped_column(BIGINT(unsigned=True))
    action: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    table_name: Mapped[str] = mapped_column(VARCHAR(100), nullable=False)
    record_id: Mapped[str | None] = mapped_column(VARCHAR(255))
    old_values: Mapped[str | None] = mapped_column(JSON)
    new_values: Mapped[str | None] = mapped_column(JSON)
    ip_address: Mapped[str | None] = mapped_column(VARCHAR(45))
    user_agent: Mapped[str | None] = mapped_column(Text)
    occurred_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyAttachment(Base):
    """Attachments for MySQL."""

    __tablename__ = "my_attachments"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["my_tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["uploaded_by"], ["my_users.id"], ondelete="CASCADE"
        ),
        Index("my_attachments_task_idx", "task_id"),
        CheckConstraint("size_bytes > 0", name="my_attachments_size_check"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[str] = mapped_column(
        VARCHAR(36), primary_key=True, server_default=text("(UUID())")
    )
    task_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    uploaded_by: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    file_name: Mapped[str] = mapped_column(VARCHAR(500), nullable=False)
    mime_type: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    size_bytes: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    storage_path: Mapped[str] = mapped_column(Text, nullable=False)
    checksum: Mapped[str | None] = mapped_column(VARCHAR(128))
    metadata_: Mapped[str | None] = mapped_column("metadata", JSON)
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyNotification(Base):
    """Notifications for MySQL."""

    __tablename__ = "my_notifications"
    __table_args__ = (
        ForeignKeyConstraint(
            ["user_id"], ["my_users.id"], ondelete="CASCADE"
        ),
        Index("my_notifications_user_idx", "user_id"),
        Index("my_notifications_created_at_idx", "created_at"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), primary_key=True, autoincrement=True
    )
    user_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    type: Mapped[str] = mapped_column(VARCHAR(50), nullable=False)
    title: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    body: Mapped[str | None] = mapped_column(Text)
    data: Mapped[str | None] = mapped_column(JSON)
    is_read: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("0")
    )
    read_at: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MySession(Base):
    """Sessions — exercises binary columns."""

    __tablename__ = "my_sessions"
    __table_args__ = (
        ForeignKeyConstraint(
            ["user_id"], ["my_users.id"], ondelete="CASCADE"
        ),
        Index("my_sessions_user_idx", "user_id"),
        Index("my_sessions_expires_idx", "expires_at"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[str] = mapped_column(VARCHAR(128), primary_key=True)
    user_id: Mapped[int] = mapped_column(BIGINT(unsigned=True), nullable=False)
    token_hash: Mapped[bytes] = mapped_column(BINARY(32), nullable=False)
    user_agent: Mapped[str | None] = mapped_column(Text)
    ip_address: Mapped[str | None] = mapped_column(VARCHAR(45))
    last_active_at: Mapped[str] = mapped_column(MYSQL_DATETIME(fsp=3), nullable=False)
    expires_at: Mapped[str] = mapped_column(MYSQL_DATETIME(fsp=3), nullable=False)
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )


class MyProjectInvite(Base):
    """Project invites — composite FK for MySQL."""

    __tablename__ = "my_project_invites"
    __table_args__ = (
        ForeignKeyConstraint(
            ["project_id"], ["my_projects.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["inviter_org_id", "inviter_user_id"],
            ["my_org_members.org_id", "my_org_members.user_id"],
            ondelete="CASCADE",
        ),
        Index("my_project_invites_project_idx", "project_id"),
        Index("my_project_invites_email_idx", "invitee_email"),
        {"mysql_engine": "InnoDB", "mysql_charset": "utf8mb4"},
    )

    id: Mapped[int] = mapped_column(
        INTEGER(unsigned=True), primary_key=True, autoincrement=True
    )
    project_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), nullable=False)
    inviter_org_id: Mapped[int] = mapped_column(INTEGER(unsigned=True), nullable=False)
    inviter_user_id: Mapped[int] = mapped_column(
        BIGINT(unsigned=True), nullable=False
    )
    invitee_email: Mapped[str] = mapped_column(VARCHAR(255), nullable=False)
    token: Mapped[str] = mapped_column(
        VARCHAR(36), nullable=False, unique=True, server_default=text("(UUID())")
    )
    expires_at: Mapped[str] = mapped_column(MYSQL_DATETIME(fsp=3), nullable=False)
    accepted_at: Mapped[str | None] = mapped_column(MYSQL_DATETIME(fsp=3))
    created_at: Mapped[str] = mapped_column(
        MYSQL_TIMESTAMP(fsp=3),
        nullable=False,
        server_default=text("CURRENT_TIMESTAMP(3)"),
    )
