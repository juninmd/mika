import { AuroraField, GlassToggle } from '@mika/sdk-ui/settings'
import type { useI18n } from '../../../renderer/src/hooks/useI18n'
import { ArgsEditor, EnvEditor, errorText } from './mcp-editor-fields'
import type { McpFormErrors, McpServerForm } from './mcp-server-form'
import type { McpTemplate } from './mcp-templates'

type T = ReturnType<typeof useI18n>['t']

type FieldsProps = {
  form: McpServerForm
  errors: McpFormErrors
  update: (patch: Partial<McpServerForm>) => void
  setCommand: (command: string) => void
  template?: McpTemplate
  t: T
}

/** Local (processo via stdio) ou remoto (Streamable HTTP): cada um mostra só os campos que usa. */
export function TransportFields(props: FieldsProps) {
  const { form, update, t } = props
  return (
    <>
      <GlassToggle
        label={t('settings.mcp.form.remote')}
        description={t('settings.mcp.form.remoteHint')}
        value={form.remote}
        onChange={(remote) => update({ remote })}
      />
      {form.remote ? <RemoteFields {...props} /> : <LocalFields {...props} />}
    </>
  )
}

function RemoteFields({ form, errors, update, t }: FieldsProps) {
  return (
    <>
      <AuroraField
        label={t('settings.mcp.form.url')}
        description={t('settings.mcp.form.urlHint')}
        value={form.url}
        onChange={(url) => update({ url })}
        placeholder={t('settings.mcp.form.urlPlaceholder')}
        warning={errorText(t, errors.url)}
        invalid={Boolean(errors.url)}
      />
      <EnvEditor kind="headers" rows={form.headers} onChange={(headers) => update({ headers })} errors={errors} t={t} />
    </>
  )
}

function LocalFields({ form, errors, update, setCommand, template, t }: FieldsProps) {
  return (
    <>
      <AuroraField
        label={t('settings.mcp.form.command')}
        description={t('settings.mcp.form.commandHint')}
        value={form.command}
        onChange={setCommand}
        placeholder={t('settings.mcp.form.commandPlaceholder')}
        warning={errorText(t, errors.command)}
        invalid={Boolean(errors.command)}
      />
      <ArgsEditor
        args={form.args}
        onChange={(args) => update({ args })}
        placeholder={
          template?.need === 'folder' ? t('settings.mcp.form.folderPlaceholder') : t('settings.mcp.form.argPlaceholder')
        }
        error={errorText(t, errors.args)}
        t={t}
      />
      <EnvEditor
        rows={form.env}
        onChange={(env) => update({ env })}
        errors={errors}
        keyUrl={template?.need === 'apiKey' ? template.docsUrl : undefined}
        t={t}
      />
      <AuroraField
        label={t('settings.mcp.form.cwd')}
        description={t('settings.mcp.form.cwdHint')}
        badge={t('settings.mcp.form.optional')}
        value={form.cwd}
        onChange={(cwd) => update({ cwd })}
        placeholder={t('settings.mcp.form.cwdPlaceholder')}
        warning={errorText(t, errors.cwd)}
        invalid={Boolean(errors.cwd)}
      />
    </>
  )
}
