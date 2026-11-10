export const USER_PROFILE_KEYS = [
  'name',
  'nickname',
  'birthDate',
  'age',
  'gender',
  'country',
  'state',
  'city',
  'timezone',
  'profession',
  'company',
  'role',
  'skills',
  'hobbies',
  'favoriteGames',
  'favoriteAnime',
  'favoriteMusic',
  'favoriteFood',
  'partnerName',
  'partnerRelationship',
  'partnerLikes',
  'partnerNotes',
  'personalityTraits',
  'communicationStyle',
  'communicationNotes',
  'bio',
  'notes',
  'learnedByLLM',
] as const

export type UserProfileKey = (typeof USER_PROFILE_KEYS)[number]

export const USER_PROFILE_FILLABLE_KEYS = USER_PROFILE_KEYS.filter(
  (key): key is Exclude<UserProfileKey, 'learnedByLLM'> => key !== 'learnedByLLM',
)
