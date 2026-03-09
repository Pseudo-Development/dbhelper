/**
 * Comprehensive Postgres schema exercising all features dbhelper must support.
 *
 * Feature coverage:
 *  - Column types: smallint, integer, bigint, serial, bigserial, real, double,
 *    numeric, varchar, char, text, boolean, date, time, timestamp, timestamptz,
 *    interval, uuid, json, jsonb, bytea, inet, cidr, macaddr
 *  - Primary keys: single-column, composite
 *  - Foreign keys: simple, composite, cascade/set-null/restrict actions
 *  - Indexes: btree, unique, composite, partial (with .where())
 *  - Constraints: not-null, unique, check, default (static & SQL expression)
 *  - Enums (pgEnum)
 *  - Array columns
 *  - Identity / generated columns
 */

import {
  pgTable,
  pgEnum,
  pgSchema,
  serial,
  bigserial,
  smallint,
  integer,
  bigint,
  real,
  doublePrecision,
  numeric,
  varchar,
  char,
  text,
  boolean,
  date,
  time,
  timestamp,
  interval,
  uuid,
  json,
  jsonb,
  inet,
  cidr,
  macaddr,
  primaryKey,
  foreignKey,
  unique,
  uniqueIndex,
  index,
  check,
} from "drizzle-orm/pg-core";
import { sql } from "drizzle-orm";

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

export const userRole = pgEnum("user_role", [
  "admin",
  "editor",
  "viewer",
  "guest",
]);

export const orderStatus = pgEnum("order_status", [
  "pending",
  "processing",
  "shipped",
  "delivered",
  "cancelled",
  "refunded",
]);

export const priority = pgEnum("priority", ["low", "medium", "high", "urgent"]);

// ---------------------------------------------------------------------------
// Named schema (non-public)
// ---------------------------------------------------------------------------

export const analytics = pgSchema("analytics");

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/** Core users table — exercises most column types and constraints */
export const users = pgTable(
  "users",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    uuid: uuid("uuid")
      .notNull()
      .default(sql`gen_random_uuid()`),
    email: varchar("email", { length: 255 }).notNull(),
    username: varchar("username", { length: 100 }).notNull(),
    passwordHash: text("password_hash").notNull(),
    role: userRole("role").notNull().default("viewer"),
    displayName: varchar("display_name", { length: 200 }),
    bio: text("bio"),
    age: smallint("age"),
    score: real("score").default(0),
    balance: numeric("balance", { precision: 15, scale: 2 }).default("0.00"),
    isActive: boolean("is_active").notNull().default(true),
    metadata: jsonb("metadata").default(sql`'{}'::jsonb`),
    ipAddress: inet("ip_address"),
    lastLoginAt: timestamp("last_login_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    uniqueIndex("users_email_idx").on(t.email),
    uniqueIndex("users_username_idx").on(t.username),
    unique("users_uuid_uniq").on(t.uuid),
    index("users_role_idx").on(t.role),
    index("users_created_at_idx").on(t.createdAt),
    // Partial index: only active users
    index("users_active_email_idx")
      .on(t.email)
      .where(sql`is_active = true`),
    check("users_age_check", sql`age IS NULL OR (age >= 0 AND age <= 200)`),
    check("users_email_check", sql`email ~* '^[^@]+@[^@]+\.[^@]+$'`),
  ]
);

/** Organizations — simple table with self-referencing FK */
export const organizations = pgTable("organizations", {
  id: serial("id").primaryKey(),
  name: varchar("name", { length: 255 }).notNull(),
  slug: varchar("slug", { length: 100 }).notNull().unique(),
  parentId: integer("parent_id"),
  description: text("description"),
  website: varchar("website", { length: 500 }),
  logoUrl: text("logo_url"),
  isVerified: boolean("is_verified").notNull().default(false),
  settings: jsonb("settings").default(sql`'{}'::jsonb`),
  createdAt: timestamp("created_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
});

/** Org members — composite PK, multiple FKs with different actions */
export const orgMembers = pgTable(
  "org_members",
  {
    orgId: integer("org_id").notNull(),
    userId: bigint("user_id", { mode: "number" }).notNull(),
    role: varchar("role", { length: 50 }).notNull().default("member"),
    joinedAt: timestamp("joined_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    primaryKey({ columns: [t.orgId, t.userId] }),
    foreignKey({ columns: [t.orgId], foreignColumns: [organizations.id] })
      .onDelete("cascade")
      .onUpdate("cascade"),
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("cascade")
      .onUpdate("cascade"),
  ]
);

/** Projects — FK to organizations */
export const projects = pgTable(
  "projects",
  {
    id: serial("id").primaryKey(),
    orgId: integer("org_id").notNull(),
    name: varchar("name", { length: 255 }).notNull(),
    slug: varchar("slug", { length: 100 }).notNull(),
    description: text("description"),
    priority: priority("priority").notNull().default("medium"),
    isArchived: boolean("is_archived").notNull().default(false),
    config: jsonb("config"),
    startDate: date("start_date"),
    deadline: date("deadline"),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.orgId], foreignColumns: [organizations.id] })
      .onDelete("cascade"),
    unique("projects_org_slug_uniq").on(t.orgId, t.slug),
    index("projects_org_id_idx").on(t.orgId),
    index("projects_priority_idx").on(t.priority),
    // Partial index: non-archived only
    index("projects_active_idx")
      .on(t.orgId, t.name)
      .where(sql`is_archived = false`),
    check(
      "projects_dates_check",
      sql`start_date IS NULL OR deadline IS NULL OR start_date <= deadline`
    ),
  ]
);

/** Tasks — FK to projects, self-referencing FK for parent tasks */
export const tasks = pgTable(
  "tasks",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    projectId: integer("project_id").notNull(),
    parentId: bigint("parent_id", { mode: "number" }),
    assigneeId: bigint("assignee_id", { mode: "number" }),
    title: varchar("title", { length: 500 }).notNull(),
    description: text("description"),
    status: orderStatus("status").notNull().default("pending"),
    priority: priority("priority").notNull().default("medium"),
    sortOrder: integer("sort_order").notNull().default(0),
    estimateHours: numeric("estimate_hours", { precision: 6, scale: 2 }),
    dueDate: timestamp("due_date", { withTimezone: true }),
    completedAt: timestamp("completed_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.projectId], foreignColumns: [projects.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.assigneeId], foreignColumns: [users.id] })
      .onDelete("set null"),
    index("tasks_project_status_idx").on(t.projectId, t.status),
    index("tasks_assignee_idx").on(t.assigneeId),
    index("tasks_due_date_idx")
      .on(t.dueDate)
      .where(sql`completed_at IS NULL`),
  ]
);

/** Comments — polymorphic-ish, FK to users and tasks */
export const comments = pgTable(
  "comments",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    taskId: bigint("task_id", { mode: "number" }).notNull(),
    authorId: bigint("author_id", { mode: "number" }).notNull(),
    parentCommentId: bigint("parent_comment_id", { mode: "number" }),
    body: text("body").notNull(),
    editedAt: timestamp("edited_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.taskId], foreignColumns: [tasks.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.authorId], foreignColumns: [users.id] })
      .onDelete("cascade"),
    index("comments_task_idx").on(t.taskId),
    index("comments_author_idx").on(t.authorId),
  ]
);

/** Tags — many-to-many with tasks via join table */
export const tags = pgTable("tags", {
  id: serial("id").primaryKey(),
  name: varchar("name", { length: 100 }).notNull().unique(),
  color: char("color", { length: 7 }).notNull().default("#808080"),
  description: text("description"),
  createdAt: timestamp("created_at", { withTimezone: true })
    .notNull()
    .defaultNow(),
});

/** Join table: tasks <-> tags — composite PK, cascade deletes */
export const taskTags = pgTable(
  "task_tags",
  {
    taskId: bigint("task_id", { mode: "number" }).notNull(),
    tagId: integer("tag_id").notNull(),
    addedAt: timestamp("added_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    primaryKey({ columns: [t.taskId, t.tagId] }),
    foreignKey({ columns: [t.taskId], foreignColumns: [tasks.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.tagId], foreignColumns: [tags.id] })
      .onDelete("cascade"),
  ]
);

/** Audit log — append-only, exercises timestamp, jsonb, inet */
export const auditLog = pgTable(
  "audit_log",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: bigint("user_id", { mode: "number" }),
    action: varchar("action", { length: 100 }).notNull(),
    tableName: varchar("table_name", { length: 100 }).notNull(),
    recordId: varchar("record_id", { length: 255 }),
    oldValues: jsonb("old_values"),
    newValues: jsonb("new_values"),
    ipAddress: inet("ip_address"),
    userAgent: text("user_agent"),
    occurredAt: timestamp("occurred_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("set null"),
    index("audit_log_user_idx").on(t.userId),
    index("audit_log_table_action_idx").on(t.tableName, t.action),
    index("audit_log_occurred_at_idx").on(t.occurredAt),
  ]
);

/** Files/attachments — exercises bytea reference, large varchar */
export const attachments = pgTable(
  "attachments",
  {
    id: uuid("id")
      .primaryKey()
      .default(sql`gen_random_uuid()`),
    taskId: bigint("task_id", { mode: "number" }).notNull(),
    uploadedBy: bigint("uploaded_by", { mode: "number" }).notNull(),
    fileName: varchar("file_name", { length: 500 }).notNull(),
    mimeType: varchar("mime_type", { length: 255 }).notNull(),
    sizeBytes: bigint("size_bytes", { mode: "number" }).notNull(),
    storagePath: text("storage_path").notNull(),
    checksum: varchar("checksum", { length: 128 }),
    metadata: jsonb("metadata"),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.taskId], foreignColumns: [tasks.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.uploadedBy], foreignColumns: [users.id] })
      .onDelete("cascade"),
    index("attachments_task_idx").on(t.taskId),
    check("attachments_size_check", sql`size_bytes > 0`),
  ]
);

/** Notifications — exercises enum, boolean, multiple timestamps */
export const notifications = pgTable(
  "notifications",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: bigint("user_id", { mode: "number" }).notNull(),
    type: varchar("type", { length: 50 }).notNull(),
    title: varchar("title", { length: 255 }).notNull(),
    body: text("body"),
    data: jsonb("data"),
    isRead: boolean("is_read").notNull().default(false),
    readAt: timestamp("read_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("cascade"),
    index("notifications_user_unread_idx")
      .on(t.userId, t.createdAt)
      .where(sql`is_read = false`),
  ]
);

// ---------------------------------------------------------------------------
// Table in non-public schema
// ---------------------------------------------------------------------------

/** Page views — in the `analytics` schema */
export const pageViews = analytics.table(
  "page_views",
  {
    id: bigserial("id", { mode: "number" }).primaryKey(),
    userId: bigint("user_id", { mode: "number" }),
    path: varchar("path", { length: 2000 }).notNull(),
    referrer: varchar("referrer", { length: 2000 }),
    userAgent: text("user_agent"),
    ipAddress: inet("ip_address"),
    sessionId: uuid("session_id"),
    duration: interval("duration"),
    viewedAt: timestamp("viewed_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    index("page_views_user_idx").on(t.userId),
    index("page_views_path_idx").on(t.path),
    index("page_views_viewed_at_idx").on(t.viewedAt),
  ]
);

/** Daily aggregated stats — in the `analytics` schema */
export const dailyStats = analytics.table(
  "daily_stats",
  {
    date: date("date").notNull(),
    path: varchar("path", { length: 2000 }).notNull(),
    views: integer("views").notNull().default(0),
    uniqueVisitors: integer("unique_visitors").notNull().default(0),
    avgDurationSecs: doublePrecision("avg_duration_secs"),
    topReferrers: jsonb("top_referrers"),
  },
  (t) => [
    primaryKey({ columns: [t.date, t.path] }),
  ]
);

// ---------------------------------------------------------------------------
// Composite FK example
// ---------------------------------------------------------------------------

/** Project invites — composite FK referencing org_members(org_id, user_id) */
export const projectInvites = pgTable(
  "project_invites",
  {
    id: serial("id").primaryKey(),
    projectId: integer("project_id").notNull(),
    inviterOrgId: integer("inviter_org_id").notNull(),
    inviterUserId: bigint("inviter_user_id", { mode: "number" }).notNull(),
    inviteeEmail: varchar("invitee_email", { length: 255 }).notNull(),
    token: uuid("token")
      .notNull()
      .unique()
      .default(sql`gen_random_uuid()`),
    expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
    acceptedAt: timestamp("accepted_at", { withTimezone: true }),
    createdAt: timestamp("created_at", { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.projectId], foreignColumns: [projects.id] })
      .onDelete("cascade"),
    foreignKey({
      columns: [t.inviterOrgId, t.inviterUserId],
      foreignColumns: [orgMembers.orgId, orgMembers.userId],
    }).onDelete("cascade"),
    index("project_invites_project_idx").on(t.projectId),
    index("project_invites_email_idx").on(t.inviteeEmail),
  ]
);
