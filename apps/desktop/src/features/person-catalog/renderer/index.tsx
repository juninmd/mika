import { defineMikaUI } from '@mika/sdk-ui'
import { PersonCatalogCRUDPanel } from './panels/person-catalog-crud.panel'
import { PersonCatalogListingView } from './views/person-catalog-listing.view'

export default defineMikaUI({
  views: {
    catalogListing: PersonCatalogListingView,
  },
  settingsPanels: {
    catalogCRUD: PersonCatalogCRUDPanel,
  },
})
