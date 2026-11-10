import { AuroraField, BentoGrid, GlassSection, GlassSelect } from '@mika/sdk-ui/settings'
import { Brain, BriefcaseBusiness, Heart, MapPin, NotebookText, Sparkles, User } from 'lucide-react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import {
  buildCommunicationOptions,
  buildGenderOptions,
  buildRelationshipOptions,
  type FieldOpts,
  resolveUserProps,
  type UserProfilePanelProps,
} from './user-profile-config'
import { ProfileSection, ProfileSnapshot } from './user-profile-sections'

export function UserProfilePanel(rawProps: UserProfilePanelProps) {
  const { t } = useI18n()
  const { props, config, completion } = resolveUserProps(rawProps)
  const field = (configPath: string, label: string, opts?: FieldOpts) => (
    <AuroraField label={label} configPath={configPath} config={config} onSave={props.onSave} {...opts} />
  )
  const genderOptions = buildGenderOptions(t)
  const relationshipOptions = buildRelationshipOptions(t)
  const communicationOptions = buildCommunicationOptions(t)

  return (
    <GlassSection title={t('settings.user.panel.title')} icon={User} description={t('settings.user.panel.description')}>
      <BentoGrid cols="auto">
        <ProfileSnapshot props={props} completion={completion} />

        <ProfileSection
          title={t('settings.user.group.identity')}
          description={t('settings.user.panel.identityDescription')}
          icon={User}
        >
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <div className="lg:col-span-2">
              {field('user.name', t('settings.user.field.name.label'), {
                placeholder: t('settings.user.field.name.placeholder'),
              })}
            </div>
            <div className="lg:col-span-2">
              {field('user.nickname', t('settings.user.field.nickname.label'), {
                placeholder: t('settings.user.field.nickname.placeholder'),
              })}
            </div>
            {field('user.birthDate', t('settings.user.field.birthDate.label'), { placeholder: 'DD/MM' })}
            {field('user.age', t('settings.user.field.age.label'), { type: 'number', placeholder: '30' })}
            <div className="sm:col-span-2 lg:col-span-4">
              <GlassSelect
                label={t('settings.user.field.gender.label')}
                configPath="user.gender"
                config={config}
                options={genderOptions}
                onSave={props.onSave as (path: string, value: string) => Promise<void>}
              />
            </div>
          </div>
        </ProfileSection>

        <ProfileSection
          title={t('settings.user.panel.locationTitle')}
          description={t('settings.user.panel.locationDescription')}
          icon={MapPin}
        >
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {field('user.country', t('settings.user.field.country.label'), {
              placeholder: t('settings.user.field.country.placeholder'),
            })}
            {field('user.state', t('settings.user.field.state.label'), { placeholder: 'SP' })}
            {field('user.city', t('settings.user.field.city.label'), {
              placeholder: t('settings.user.field.city.placeholder'),
            })}
            {field('user.timezone', t('settings.user.panel.timezoneLabel'), { placeholder: 'America/Sao_Paulo' })}
          </div>
          <div className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {field('user.profession', t('settings.user.field.profession.label'), {
              placeholder: t('settings.user.field.profession.placeholder'),
            })}
            {field('user.company', t('settings.user.field.company.label'), {
              placeholder: t('settings.user.field.company.placeholder'),
            })}
            {field('user.role', t('settings.user.field.role.label'), {
              placeholder: t('settings.user.field.role.placeholder'),
            })}
            {field('user.skills', t('settings.user.field.skills.label'), {
              placeholder: t('settings.user.field.skills.placeholder'),
            })}
          </div>
        </ProfileSection>

        <ProfileSection
          title={t('settings.user.panel.interestsTitle')}
          description={t('settings.user.panel.interestsDescription')}
          icon={Sparkles}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            {field('user.hobbies', t('settings.user.panel.hobbiesLabel'), {
              type: 'textarea',
              rows: 2,
              placeholder: 'games, anime, código...',
            })}
            {field('user.favoriteGames', t('settings.user.field.favoriteGames.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.favoriteGames.placeholder'),
            })}
            {field('user.favoriteAnime', t('settings.user.field.favoriteAnime.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.favoriteAnime.placeholder'),
            })}
            {field('user.favoriteMusic', t('settings.user.field.favoriteMusic.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.favoriteMusic.placeholder'),
            })}
            {field('user.favoriteFood', t('settings.user.field.favoriteFood.label'), {
              placeholder: t('settings.user.field.favoriteFood.placeholder'),
            })}
          </div>
        </ProfileSection>

        <ProfileSection
          title={t('settings.user.panel.peopleTitle')}
          description={t('settings.user.panel.peopleDescription')}
          icon={Heart}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            {field('user.partnerName', t('settings.user.panel.partnerNameLabel'), {
              placeholder: t('settings.user.panel.partnerNamePlaceholder'),
            })}
            <GlassSelect
              label={t('settings.user.field.partnerRelationship.label')}
              configPath="user.partnerRelationship"
              config={config}
              options={relationshipOptions}
              onSave={props.onSave as (path: string, value: string) => Promise<void>}
            />
            {field('user.partnerLikes', t('settings.user.field.partnerLikes.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.partnerLikes.placeholder'),
            })}
            {field('user.partnerNotes', t('settings.user.field.partnerNotes.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.partnerNotes.placeholder'),
            })}
          </div>
        </ProfileSection>

        <ProfileSection
          title={t('settings.user.panel.personalityTitle')}
          description={t('settings.user.panel.personalityDescription')}
          icon={BriefcaseBusiness}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="sm:col-span-2">
              <GlassSelect
                label={t('settings.user.field.communicationStyle.label')}
                description={t('settings.user.field.communicationStyle.description')}
                configPath="user.communicationStyle"
                config={config}
                options={communicationOptions}
                onSave={props.onSave as (path: string, value: string) => Promise<void>}
              />
            </div>
            {field('user.personalityTraits', t('settings.user.field.personalityTraits.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.personalityTraits.placeholder'),
            })}
            {field('user.communicationNotes', t('settings.user.field.communicationNotes.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.user.field.communicationNotes.placeholder'),
            })}
          </div>
        </ProfileSection>

        <ProfileSection
          title={t('settings.user.panel.bioTitle')}
          description={t('settings.user.panel.bioSectionDescription')}
          icon={NotebookText}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            {field('user.bio', t('settings.user.field.bio.label'), {
              type: 'textarea',
              rows: 3,
              placeholder: t('settings.user.field.bio.placeholder'),
            })}
            {field('user.notes', t('settings.user.field.notes.label'), {
              type: 'textarea',
              rows: 3,
              placeholder: t('settings.user.field.notes.placeholder'),
            })}
            <div className="rounded-xl border border-border-subtle bg-bg-surface p-3 sm:col-span-2">
              <div className="mb-2 flex items-center gap-2 text-micro font-black uppercase tracking-[0.14em] text-text-muted">
                <Brain size={14} aria-hidden="true" />
                {t('settings.user.panel.learnedAutomatically')}
              </div>
              {/* Somente leitura: sem opacity no bloco todo, ou o texto preenchido pela Mika fica ilegível. */}
              <div className="pointer-events-none cursor-not-allowed">
                {field('user.learnedByLLM', t('settings.user.field.learnedByLLM.label'), {
                  type: 'textarea',
                  rows: 3,
                  placeholder: t('settings.user.field.learnedByLLM.placeholder'),
                })}
              </div>
            </div>
          </div>
        </ProfileSection>
      </BentoGrid>
    </GlassSection>
  )
}
