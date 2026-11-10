export type Recurrence = 'once' | 'daily'

export type Reminder = {
  id: string
  subject: string
  dueAt: number
  recurrence: Recurrence
}

/** What the form is pre-filled with: a preset chip from the empty state, or a reminder being edited. */
export type ReminderDraft = { subject: string; atTime: string; recurrence: Recurrence; editId?: string }

export type ReminderToolResult = { success?: boolean; data?: { reminders?: Reminder[] }; error?: string }
export type ScheduleToolResult = { success?: boolean; error?: string }
