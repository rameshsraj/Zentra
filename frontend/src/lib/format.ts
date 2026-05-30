import { format, formatDistanceToNow } from 'date-fns'

export function formatDate(date: string | Date | null | undefined): string {
  if (!date) return '—'
  const d = typeof date === 'string' ? new Date(date) : date
  if (isNaN(d.getTime())) return '—'
  return format(d, 'MMM d, yyyy HH:mm')
}

export function formatRelative(date: string | Date | null | undefined): string {
  if (!date) return '—'
  const d = typeof date === 'string' ? new Date(date) : date
  if (isNaN(d.getTime())) return '—'
  return formatDistanceToNow(d, { addSuffix: true })
}

export function formatNumber(value: number | null | undefined): string {
  if (value == null) return '—'
  return new Intl.NumberFormat('en-US').format(value)
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

export function formatPercent(value: number | null | undefined): string {
  if (value == null) return '—'
  return `${value.toFixed(1)}%`
}

/** Map browser locale to ISO 4217 currency code */
function detectCurrency(): string {
  const locale = navigator.language || 'en-US'
  const region = locale.split('-')[1]?.toUpperCase() ?? ''
  const map: Record<string, string> = {
    IN: 'INR', US: 'USD', GB: 'GBP', AU: 'AUD', CA: 'CAD',
    JP: 'JPY', CN: 'CNY', KR: 'KRW', SG: 'SGD', AE: 'AED',
    SA: 'SAR', DE: 'EUR', FR: 'EUR', IT: 'EUR', ES: 'EUR',
    NL: 'EUR', BE: 'EUR', AT: 'EUR', IE: 'EUR', PT: 'EUR',
    FI: 'EUR', GR: 'EUR', BR: 'BRL', MX: 'MXN', ZA: 'ZAR',
    SE: 'SEK', NO: 'NOK', DK: 'DKK', CH: 'CHF', NZ: 'NZD',
    HK: 'HKD', TW: 'TWD', TH: 'THB', MY: 'MYR', PH: 'PHP',
    ID: 'IDR', VN: 'VND', PK: 'PKR', BD: 'BDT', LK: 'LKR',
  }
  return map[region] || 'USD'
}

const _currencyCode = detectCurrency()
const _locale = navigator.language || 'en-US'

/** Format a number as currency using the browser's locale */
export function formatCurrency(value: number | null | undefined, decimals?: number): string {
  if (value == null) return '—'
  return new Intl.NumberFormat(_locale, {
    style: 'currency',
    currency: _currencyCode,
    minimumFractionDigits: decimals ?? 0,
    maximumFractionDigits: decimals ?? 2,
  }).format(value)
}

/** Get the currency symbol for the current locale (e.g. ₹, $, €, £) */
export function getCurrencySymbol(): string {
  return new Intl.NumberFormat(_locale, { style: 'currency', currency: _currencyCode })
    .formatToParts(0)
    .find(p => p.type === 'currency')?.value ?? '$'
}
