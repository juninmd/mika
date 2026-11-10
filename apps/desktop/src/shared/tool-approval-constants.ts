// Todo diálogo de aprovação é sobre uma ação irreversível (ver ToolApprovalDialog.tsx,
// shell.approval.irreversible é exibido incondicionalmente) — por isso o piso vale para todas.
export const TOOL_APPROVAL_COUNTDOWN_SECONDS = 60

const TOOL_APPROVAL_BACKSTOP_MARGIN_SECONDS = 5

export const TOOL_APPROVAL_BACKSTOP_MS =
  (TOOL_APPROVAL_COUNTDOWN_SECONDS + TOOL_APPROVAL_BACKSTOP_MARGIN_SECONDS) * 1000
