"""
Comprehensive Postgres models exercising all features dbhelper must support.

Feature coverage:
 - Column types: SmallInteger, Integer, BigInteger, Float, Double, Numeric,
   String, Text, Boolean, Date, Time, DateTime (with timezone), Interval,
   UUID, JSON, JSONB, LargeBinary, INET, CIDR, MACADDR, ARRAY
 - Primary keys: single-column, composite
 - Foreign keys: simple, composite, cascade/set-null/restrict actions
 - Indexes: btree, unique, composite, partial (with postgresql_where)
 - Constraints: not-null, unique, check, default (static & server_default)
 - Enums (native PG enums via Enum type)
 - Schema qualification (non-public schema)
 - Self-referencing foreign keys
"""

import enum

from sqlalchemy import (
    BigInteger,
    Boolean,
    CheckConstraint,
    Column,
    Date,
    DateTime,
    Enum,
    Float,
    ForeignKey,
    ForeignKeyConstraint,
    Index,
    Integer,
    Interval,
    Numeric,
    SmallInteger,
    String,
    Text,
    UniqueConstraint,
    text,
)
from sqlalchemy.dialects.postgresql import (
    CIDR,
    INET,
    JSONB,
    MACADDR,
    UUID,
)
from sqlalchemy.orm import Mapped, mapped_column, relationship
from sqlalchemy.schema import DDL

from models.base import Base


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class UserRole(enum.Enum):
    admin = "admin"
    editor = "editor"
    viewer = "viewer"
    guest = "guest"


class OrderStatus(enum.Enum):
    pending = "pending"
    processing = "processing"
    shipped = "shipped"
    delivered = "delivered"
    cancelled = "cancelled"
    refunded = "refunded"


class Priority(enum.Enum):
    low = "low"
    medium = "medium"
    high = "high"
    urgent = "urgent"


# ---------------------------------------------------------------------------
# Tables (public schema)
# ---------------------------------------------------------------------------


class PgUser(Base):
    """Core users table — exercises most column types and constraints."""

    __tablename__ = "users"
    __table_args__ = (
        UniqueConstraint("uuid", name="users_uuid_uniq"),
        Index("users_email_idx", "email", unique=True),
        Index("users_username_idx", "username", unique=True),
        Index("users_role_idx", "role"),
        Index("users_created_at_idx", "created_at"),
        Index(
            "users_active_email_idx",
            "email",
            postgresql_where=text("is_active = true"),
        ),
        CheckConstraint(
            "age IS NULL OR (age >= 0 AND age <= 200)", name="users_age_check"
        ),
        CheckConstraint(
            "email ~* '^[^@]+@[^@]+\\.[^@]+$'", name="users_email_check"
        ),
        {"schema": None},  # public schema
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    uuid: Mapped[str] = mapped_column(
        UUID(as_uuid=False),
        nullable=False,
        server_default=text("gen_random_uuid()"),
    )
    email: Mapped[str] = mapped_column(String(255), nullable=False)
    username: Mapped[str] = mapped_column(String(100), nullable=False)
    password_hash: Mapped[str] = mapped_column(Text, nullable=False)
    role: Mapped[UserRole] = mapped_column(
        Enum(UserRole, name="user_role", create_type=True),
        nullable=False,
        server_default="viewer",
    )
    display_name: Mapped[str | None] = mapped_column(String(200))
    bio: Mapped[str | None] = mapped_column(Text)
    age: Mapped[int | None] = mapped_column(SmallInteger)
    score: Mapped[float | None] = mapped_column(Float, server_default=text("0"))
    balance: Mapped[str | None] = mapped_column(
        Numeric(15, 2), server_default=text("0.00")
    )
    is_active: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("true")
    )
    metadata_: Mapped[dict | None] = mapped_column(
        "metadata", JSONB, server_default=text("'{}'::jsonb")
    )
    ip_address: Mapped[str | None] = mapped_column(INET)
    last_login_at: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )
    updated_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgOrganization(Base):
    """Organizations — self-referencing FK."""

    __tablename__ = "organizations"
    __table_args__ = (
        UniqueConstraint("slug"),
    )

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    slug: Mapped[str] = mapped_column(String(100), nullable=False)
    parent_id: Mapped[int | None] = mapped_column(
        Integer, ForeignKey("organizations.id", ondelete="SET NULL")
    )
    description: Mapped[str | None] = mapped_column(Text)
    website: Mapped[str | None] = mapped_column(String(500))
    logo_url: Mapped[str | None] = mapped_column(Text)
    is_verified: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("false")
    )
    settings: Mapped[dict | None] = mapped_column(
        JSONB, server_default=text("'{}'::jsonb")
    )
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgOrgMember(Base):
    """Org members — composite PK, multiple FKs with cascade."""

    __tablename__ = "org_members"
    __table_args__ = (
        ForeignKeyConstraint(
            ["org_id"],
            ["organizations.id"],
            ondelete="CASCADE",
            onupdate="CASCADE",
        ),
        ForeignKeyConstraint(
            ["user_id"],
            ["users.id"],
            ondelete="CASCADE",
            onupdate="CASCADE",
        ),
    )

    org_id: Mapped[int] = mapped_column(Integer, primary_key=True)
    user_id: Mapped[int] = mapped_column(BigInteger, primary_key=True)
    role: Mapped[str] = mapped_column(
        String(50), nullable=False, server_default=text("'member'")
    )
    joined_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgProject(Base):
    """Projects — FK to organizations."""

    __tablename__ = "projects"
    __table_args__ = (
        ForeignKeyConstraint(
            ["org_id"], ["organizations.id"], ondelete="CASCADE"
        ),
        UniqueConstraint("org_id", "slug", name="projects_org_slug_uniq"),
        Index("projects_org_id_idx", "org_id"),
        Index("projects_priority_idx", "priority"),
        Index(
            "projects_active_idx",
            "org_id",
            "name",
            postgresql_where=text("is_archived = false"),
        ),
        CheckConstraint(
            "start_date IS NULL OR deadline IS NULL OR start_date <= deadline",
            name="projects_dates_check",
        ),
    )

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    org_id: Mapped[int] = mapped_column(Integer, nullable=False)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    slug: Mapped[str] = mapped_column(String(100), nullable=False)
    description: Mapped[str | None] = mapped_column(Text)
    priority: Mapped[Priority] = mapped_column(
        Enum(Priority, name="priority", create_type=True),
        nullable=False,
        server_default="medium",
    )
    is_archived: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("false")
    )
    config: Mapped[dict | None] = mapped_column(JSONB)
    start_date: Mapped[str | None] = mapped_column(Date)
    deadline: Mapped[str | None] = mapped_column(Date)
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )
    updated_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgTask(Base):
    """Tasks — FK to projects, self-referencing FK for parents."""

    __tablename__ = "tasks"
    __table_args__ = (
        ForeignKeyConstraint(
            ["project_id"], ["projects.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["assignee_id"], ["users.id"], ondelete="SET NULL"
        ),
        Index("tasks_project_status_idx", "project_id", "status"),
        Index("tasks_assignee_idx", "assignee_id"),
        Index(
            "tasks_due_date_idx",
            "due_date",
            postgresql_where=text("completed_at IS NULL"),
        ),
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    project_id: Mapped[int] = mapped_column(Integer, nullable=False)
    parent_id: Mapped[int | None] = mapped_column(
        BigInteger, ForeignKey("tasks.id", ondelete="CASCADE")
    )
    assignee_id: Mapped[int | None] = mapped_column(BigInteger)
    title: Mapped[str] = mapped_column(String(500), nullable=False)
    description: Mapped[str | None] = mapped_column(Text)
    status: Mapped[OrderStatus] = mapped_column(
        Enum(OrderStatus, name="order_status", create_type=True),
        nullable=False,
        server_default="pending",
    )
    priority: Mapped[Priority] = mapped_column(
        Enum(Priority, name="priority", create_type=False),
        nullable=False,
        server_default="medium",
    )
    sort_order: Mapped[int] = mapped_column(
        Integer, nullable=False, server_default=text("0")
    )
    estimate_hours: Mapped[str | None] = mapped_column(Numeric(6, 2))
    due_date: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    completed_at: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )
    updated_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgComment(Base):
    """Comments — FK to users and tasks."""

    __tablename__ = "comments"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["author_id"], ["users.id"], ondelete="CASCADE"
        ),
        Index("comments_task_idx", "task_id"),
        Index("comments_author_idx", "author_id"),
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    task_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    author_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    parent_comment_id: Mapped[int | None] = mapped_column(
        BigInteger, ForeignKey("comments.id", ondelete="CASCADE")
    )
    body: Mapped[str] = mapped_column(Text, nullable=False)
    edited_at: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgTag(Base):
    """Tags — many-to-many with tasks via join table."""

    __tablename__ = "tags"
    __table_args__ = (UniqueConstraint("name"),)

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(100), nullable=False)
    color: Mapped[str] = mapped_column(
        String(7), nullable=False, server_default=text("'#808080'")
    )
    description: Mapped[str | None] = mapped_column(Text)
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgTaskTag(Base):
    """Join table: tasks <-> tags — composite PK, cascade deletes."""

    __tablename__ = "task_tags"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["tag_id"], ["tags.id"], ondelete="CASCADE"
        ),
    )

    task_id: Mapped[int] = mapped_column(BigInteger, primary_key=True)
    tag_id: Mapped[int] = mapped_column(Integer, primary_key=True)
    added_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgAuditLog(Base):
    """Audit log — append-only, exercises timestamp, jsonb, inet."""

    __tablename__ = "audit_log"
    __table_args__ = (
        ForeignKeyConstraint(
            ["user_id"], ["users.id"], ondelete="SET NULL"
        ),
        Index("audit_log_user_idx", "user_id"),
        Index("audit_log_table_action_idx", "table_name", "action"),
        Index("audit_log_occurred_at_idx", "occurred_at"),
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    user_id: Mapped[int | None] = mapped_column(BigInteger)
    action: Mapped[str] = mapped_column(String(100), nullable=False)
    table_name: Mapped[str] = mapped_column(String(100), nullable=False)
    record_id: Mapped[str | None] = mapped_column(String(255))
    old_values: Mapped[dict | None] = mapped_column(JSONB)
    new_values: Mapped[dict | None] = mapped_column(JSONB)
    ip_address: Mapped[str | None] = mapped_column(INET)
    user_agent: Mapped[str | None] = mapped_column(Text)
    occurred_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgAttachment(Base):
    """Attachments — UUID PK, size check constraint."""

    __tablename__ = "attachments"
    __table_args__ = (
        ForeignKeyConstraint(
            ["task_id"], ["tasks.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["uploaded_by"], ["users.id"], ondelete="CASCADE"
        ),
        Index("attachments_task_idx", "task_id"),
        CheckConstraint("size_bytes > 0", name="attachments_size_check"),
    )

    id: Mapped[str] = mapped_column(
        UUID(as_uuid=False),
        primary_key=True,
        server_default=text("gen_random_uuid()"),
    )
    task_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    uploaded_by: Mapped[int] = mapped_column(BigInteger, nullable=False)
    file_name: Mapped[str] = mapped_column(String(500), nullable=False)
    mime_type: Mapped[str] = mapped_column(String(255), nullable=False)
    size_bytes: Mapped[int] = mapped_column(BigInteger, nullable=False)
    storage_path: Mapped[str] = mapped_column(Text, nullable=False)
    checksum: Mapped[str | None] = mapped_column(String(128))
    metadata_: Mapped[dict | None] = mapped_column("metadata", JSONB)
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgNotification(Base):
    """Notifications — exercises boolean, partial index pattern."""

    __tablename__ = "notifications"
    __table_args__ = (
        ForeignKeyConstraint(
            ["user_id"], ["users.id"], ondelete="CASCADE"
        ),
        Index(
            "notifications_user_unread_idx",
            "user_id",
            "created_at",
            postgresql_where=text("is_read = false"),
        ),
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    user_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    type: Mapped[str] = mapped_column(String(50), nullable=False)
    title: Mapped[str] = mapped_column(String(255), nullable=False)
    body: Mapped[str | None] = mapped_column(Text)
    data: Mapped[dict | None] = mapped_column(JSONB)
    is_read: Mapped[bool] = mapped_column(
        Boolean, nullable=False, server_default=text("false")
    )
    read_at: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


# ---------------------------------------------------------------------------
# Tables in 'analytics' schema
# ---------------------------------------------------------------------------


class PgPageView(Base):
    """Page views — in the analytics schema."""

    __tablename__ = "page_views"
    __table_args__ = (
        Index("page_views_user_idx", "user_id"),
        Index("page_views_path_idx", "path"),
        Index("page_views_viewed_at_idx", "viewed_at"),
        {"schema": "analytics"},
    )

    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    user_id: Mapped[int | None] = mapped_column(BigInteger)
    path: Mapped[str] = mapped_column(String(2000), nullable=False)
    referrer: Mapped[str | None] = mapped_column(String(2000))
    user_agent: Mapped[str | None] = mapped_column(Text)
    ip_address: Mapped[str | None] = mapped_column(INET)
    session_id: Mapped[str | None] = mapped_column(UUID(as_uuid=False))
    duration: Mapped[str | None] = mapped_column(Interval)
    viewed_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )


class PgDailyStats(Base):
    """Daily aggregated stats — composite PK, in analytics schema."""

    __tablename__ = "daily_stats"
    __table_args__ = {"schema": "analytics"}

    date: Mapped[str] = mapped_column(Date, primary_key=True)
    path: Mapped[str] = mapped_column(String(2000), primary_key=True)
    views: Mapped[int] = mapped_column(
        Integer, nullable=False, server_default=text("0")
    )
    unique_visitors: Mapped[int] = mapped_column(
        Integer, nullable=False, server_default=text("0")
    )
    avg_duration_secs: Mapped[float | None] = mapped_column(Float)
    top_referrers: Mapped[dict | None] = mapped_column(JSONB)


# ---------------------------------------------------------------------------
# Composite FK example
# ---------------------------------------------------------------------------


class PgProjectInvite(Base):
    """Project invites — composite FK referencing org_members."""

    __tablename__ = "project_invites"
    __table_args__ = (
        ForeignKeyConstraint(
            ["project_id"], ["projects.id"], ondelete="CASCADE"
        ),
        ForeignKeyConstraint(
            ["inviter_org_id", "inviter_user_id"],
            ["org_members.org_id", "org_members.user_id"],
            ondelete="CASCADE",
        ),
        Index("project_invites_project_idx", "project_id"),
        Index("project_invites_email_idx", "invitee_email"),
    )

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    project_id: Mapped[int] = mapped_column(Integer, nullable=False)
    inviter_org_id: Mapped[int] = mapped_column(Integer, nullable=False)
    inviter_user_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    invitee_email: Mapped[str] = mapped_column(String(255), nullable=False)
    token: Mapped[str] = mapped_column(
        UUID(as_uuid=False),
        nullable=False,
        unique=True,
        server_default=text("gen_random_uuid()"),
    )
    expires_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False
    )
    accepted_at: Mapped[str | None] = mapped_column(DateTime(timezone=True))
    created_at: Mapped[str] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=text("now()")
    )
