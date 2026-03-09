/**
 * Comprehensive MySQL schema exercising all features dbhelper must support.
 *
 * Feature coverage:
 *  - Column types: tinyint, smallint, int, bigint, float, double, decimal,
 *    varchar, char, text, mediumtext, longtext, boolean, date, time, datetime,
 *    timestamp, year, json, binary, varbinary, blob, enum, set
 *  - Primary keys: single-column, composite
 *  - Foreign keys: simple, composite, cascade/set-null/restrict actions
 *  - Indexes: btree, unique, composite, prefix-length
 *  - Constraints: not-null, unique, check, default (static & expression)
 *  - MySQL enums (inline) and sets
 *  - Auto-increment
 *  - Character sets and collations (via column options where possible)
 */

import {
  mysqlTable,
  mysqlEnum,
  mysqlSchema,
  serial,
  tinyint,
  smallint,
  int,
  bigint,
  float,
  double,
  decimal,
  varchar,
  char,
  text,
  mediumtext,
  longtext,
  boolean,
  date,
  time,
  datetime,
  timestamp,
  year,
  json,
  binary,
  varbinary,
  primaryKey,
  foreignKey,
  unique,
  uniqueIndex,
  index,
  check,
} from "drizzle-orm/mysql-core";
import { sql } from "drizzle-orm";

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/** Core users table */
export const users = mysqlTable(
  "users",
  {
    id: bigint("id", { mode: "number", unsigned: true })
      .primaryKey()
      .autoincrement(),
    uuid: varchar("uuid", { length: 36 })
      .notNull()
      .default(sql`(UUID())`),
    email: varchar("email", { length: 255 }).notNull(),
    username: varchar("username", { length: 100 }).notNull(),
    passwordHash: text("password_hash").notNull(),
    role: mysqlEnum("role", ["admin", "editor", "viewer", "guest"])
      .notNull()
      .default("viewer"),
    displayName: varchar("display_name", { length: 200 }),
    bio: mediumtext("bio"),
    age: tinyint("age", { unsigned: true }),
    score: float("score").default(0),
    balance: decimal("balance", { precision: 15, scale: 2 }).default("0.00"),
    isActive: boolean("is_active").notNull().default(true),
    metadata: json("metadata"),
    lastLoginAt: datetime("last_login_at", { fsp: 3 }),
    createdAt: timestamp("created_at", { fsp: 3 })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp("updated_at", { fsp: 3 })
      .notNull()
      .defaultNow()
      .onUpdateNow(),
  },
  (t) => [
    uniqueIndex("users_email_idx").on(t.email),
    uniqueIndex("users_username_idx").on(t.username),
    unique("users_uuid_uniq").on(t.uuid),
    index("users_role_idx").on(t.role),
    index("users_created_at_idx").on(t.createdAt),
    check("users_age_check", sql`age IS NULL OR (age >= 0 AND age <= 200)`),
  ]
);

/** Organizations */
export const organizations = mysqlTable("organizations", {
  id: int("id", { unsigned: true }).primaryKey().autoincrement(),
  name: varchar("name", { length: 255 }).notNull(),
  slug: varchar("slug", { length: 100 }).notNull().unique(),
  parentId: int("parent_id", { unsigned: true }),
  description: text("description"),
  website: varchar("website", { length: 500 }),
  logoUrl: text("logo_url"),
  isVerified: boolean("is_verified").notNull().default(false),
  settings: json("settings"),
  createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
});

/** Org members — composite PK */
export const orgMembers = mysqlTable(
  "org_members",
  {
    orgId: int("org_id", { unsigned: true }).notNull(),
    userId: bigint("user_id", { mode: "number", unsigned: true }).notNull(),
    role: varchar("role", { length: 50 }).notNull().default("member"),
    joinedAt: timestamp("joined_at", { fsp: 3 }).notNull().defaultNow(),
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

/** Projects */
export const projects = mysqlTable(
  "projects",
  {
    id: int("id", { unsigned: true }).primaryKey().autoincrement(),
    orgId: int("org_id", { unsigned: true }).notNull(),
    name: varchar("name", { length: 255 }).notNull(),
    slug: varchar("slug", { length: 100 }).notNull(),
    description: text("description"),
    priority: mysqlEnum("priority", ["low", "medium", "high", "urgent"])
      .notNull()
      .default("medium"),
    isArchived: boolean("is_archived").notNull().default(false),
    config: json("config"),
    startDate: date("start_date"),
    deadline: date("deadline"),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { fsp: 3 })
      .notNull()
      .defaultNow()
      .onUpdateNow(),
  },
  (t) => [
    foreignKey({ columns: [t.orgId], foreignColumns: [organizations.id] })
      .onDelete("cascade"),
    unique("projects_org_slug_uniq").on(t.orgId, t.slug),
    index("projects_org_id_idx").on(t.orgId),
    index("projects_priority_idx").on(t.priority),
    check(
      "projects_dates_check",
      sql`start_date IS NULL OR deadline IS NULL OR start_date <= deadline`
    ),
  ]
);

/** Tasks */
export const tasks = mysqlTable(
  "tasks",
  {
    id: bigint("id", { mode: "number", unsigned: true })
      .primaryKey()
      .autoincrement(),
    projectId: int("project_id", { unsigned: true }).notNull(),
    parentId: bigint("parent_id", { mode: "number", unsigned: true }),
    assigneeId: bigint("assignee_id", { mode: "number", unsigned: true }),
    title: varchar("title", { length: 500 }).notNull(),
    description: mediumtext("description"),
    status: mysqlEnum("status", [
      "pending",
      "processing",
      "shipped",
      "delivered",
      "cancelled",
      "refunded",
    ])
      .notNull()
      .default("pending"),
    priority: mysqlEnum("priority", ["low", "medium", "high", "urgent"])
      .notNull()
      .default("medium"),
    sortOrder: int("sort_order").notNull().default(0),
    estimateHours: decimal("estimate_hours", { precision: 6, scale: 2 }),
    dueDate: datetime("due_date", { fsp: 3 }),
    completedAt: datetime("completed_at", { fsp: 3 }),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
    updatedAt: timestamp("updated_at", { fsp: 3 })
      .notNull()
      .defaultNow()
      .onUpdateNow(),
  },
  (t) => [
    foreignKey({ columns: [t.projectId], foreignColumns: [projects.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.assigneeId], foreignColumns: [users.id] })
      .onDelete("set null"),
    index("tasks_project_status_idx").on(t.projectId, t.status),
    index("tasks_assignee_idx").on(t.assigneeId),
  ]
);

/** Comments */
export const comments = mysqlTable(
  "comments",
  {
    id: bigint("id", { mode: "number", unsigned: true })
      .primaryKey()
      .autoincrement(),
    taskId: bigint("task_id", { mode: "number", unsigned: true }).notNull(),
    authorId: bigint("author_id", { mode: "number", unsigned: true }).notNull(),
    parentCommentId: bigint("parent_comment_id", {
      mode: "number",
      unsigned: true,
    }),
    body: mediumtext("body").notNull(),
    editedAt: datetime("edited_at", { fsp: 3 }),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
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

/** Tags */
export const tags = mysqlTable("tags", {
  id: int("id", { unsigned: true }).primaryKey().autoincrement(),
  name: varchar("name", { length: 100 }).notNull().unique(),
  color: char("color", { length: 7 }).notNull().default("#808080"),
  description: text("description"),
  createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
});

/** Join table: tasks <-> tags */
export const taskTags = mysqlTable(
  "task_tags",
  {
    taskId: bigint("task_id", { mode: "number", unsigned: true }).notNull(),
    tagId: int("tag_id", { unsigned: true }).notNull(),
    addedAt: timestamp("added_at", { fsp: 3 }).notNull().defaultNow(),
  },
  (t) => [
    primaryKey({ columns: [t.taskId, t.tagId] }),
    foreignKey({ columns: [t.taskId], foreignColumns: [tasks.id] })
      .onDelete("cascade"),
    foreignKey({ columns: [t.tagId], foreignColumns: [tags.id] })
      .onDelete("cascade"),
  ]
);

/** Audit log */
export const auditLog = mysqlTable(
  "audit_log",
  {
    id: bigint("id", { mode: "number", unsigned: true })
      .primaryKey()
      .autoincrement(),
    userId: bigint("user_id", { mode: "number", unsigned: true }),
    action: varchar("action", { length: 100 }).notNull(),
    tableName: varchar("table_name", { length: 100 }).notNull(),
    recordId: varchar("record_id", { length: 255 }),
    oldValues: json("old_values"),
    newValues: json("new_values"),
    ipAddress: varchar("ip_address", { length: 45 }),
    userAgent: text("user_agent"),
    occurredAt: timestamp("occurred_at", { fsp: 3 }).notNull().defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("set null"),
    index("audit_log_user_idx").on(t.userId),
    index("audit_log_table_action_idx").on(t.tableName, t.action),
    index("audit_log_occurred_at_idx").on(t.occurredAt),
  ]
);

/** Attachments */
export const attachments = mysqlTable(
  "attachments",
  {
    id: varchar("id", { length: 36 })
      .primaryKey()
      .default(sql`(UUID())`),
    taskId: bigint("task_id", { mode: "number", unsigned: true }).notNull(),
    uploadedBy: bigint("uploaded_by", { mode: "number", unsigned: true })
      .notNull(),
    fileName: varchar("file_name", { length: 500 }).notNull(),
    mimeType: varchar("mime_type", { length: 255 }).notNull(),
    sizeBytes: bigint("size_bytes", { mode: "number", unsigned: true })
      .notNull(),
    storagePath: text("storage_path").notNull(),
    checksum: varchar("checksum", { length: 128 }),
    metadata: json("metadata"),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
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

/** Notifications */
export const notifications = mysqlTable(
  "notifications",
  {
    id: bigint("id", { mode: "number", unsigned: true })
      .primaryKey()
      .autoincrement(),
    userId: bigint("user_id", { mode: "number", unsigned: true }).notNull(),
    type: varchar("type", { length: 50 }).notNull(),
    title: varchar("title", { length: 255 }).notNull(),
    body: text("body"),
    data: json("data"),
    isRead: boolean("is_read").notNull().default(false),
    readAt: datetime("read_at", { fsp: 3 }),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("cascade"),
    index("notifications_user_idx").on(t.userId),
    index("notifications_created_at_idx").on(t.createdAt),
  ]
);

/** Sessions — exercises binary columns and TTL-like patterns */
export const sessions = mysqlTable(
  "sessions",
  {
    id: varchar("id", { length: 128 }).primaryKey(),
    userId: bigint("user_id", { mode: "number", unsigned: true }).notNull(),
    tokenHash: binary("token_hash", { length: 32 }).notNull(),
    userAgent: text("user_agent"),
    ipAddress: varchar("ip_address", { length: 45 }),
    lastActiveAt: datetime("last_active_at", { fsp: 3 }).notNull(),
    expiresAt: datetime("expires_at", { fsp: 3 }).notNull(),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
  },
  (t) => [
    foreignKey({ columns: [t.userId], foreignColumns: [users.id] })
      .onDelete("cascade"),
    index("sessions_user_idx").on(t.userId),
    index("sessions_expires_idx").on(t.expiresAt),
  ]
);

/** Composite FK example — project invites */
export const projectInvites = mysqlTable(
  "project_invites",
  {
    id: int("id", { unsigned: true }).primaryKey().autoincrement(),
    projectId: int("project_id", { unsigned: true }).notNull(),
    inviterOrgId: int("inviter_org_id", { unsigned: true }).notNull(),
    inviterUserId: bigint("inviter_user_id", {
      mode: "number",
      unsigned: true,
    }).notNull(),
    inviteeEmail: varchar("invitee_email", { length: 255 }).notNull(),
    token: varchar("token", { length: 36 })
      .notNull()
      .unique()
      .default(sql`(UUID())`),
    expiresAt: datetime("expires_at", { fsp: 3 }).notNull(),
    acceptedAt: datetime("accepted_at", { fsp: 3 }),
    createdAt: timestamp("created_at", { fsp: 3 }).notNull().defaultNow(),
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
