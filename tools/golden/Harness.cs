// The golden harness: runs mudsort's own SortFlag.cs, Enums.cs and
// AlphanumComparator.cs, unchanged, on .NET Framework (x86, en-US: what a
// Decal player ran), and writes the files the Rust tests compare against.
// Decal's types are replaced by the stand-ins below; the sort loop and the
// sort-string parser are copied from PluginCore.cs (BUILDING_LIST,
// createSortFlagListFromString, sortFlagListToString). See run.ps1.
//
// C# 5 (the compiler that ships with .NET Framework).

using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;
using System.Threading;

namespace Decal.Adapter.Wrappers
{
    public enum LongValueKey { }
    public enum DoubleValueKey { }
    public enum StringValueKey { Name = 1 }
    public enum BoolValueKey { }

    // Decal.Adapter.Wrappers.ObjectClass, values as Decal.Adapter.dll has them.
    public enum ObjectClass
    {
        Unknown = 0, MeleeWeapon = 1, Armor = 2, Clothing = 3, Jewelry = 4, Monster = 5, Food = 6,
        Money = 7, Misc = 8, MissileWeapon = 9, Container = 10, Gem = 11, SpellComponent = 12,
        Key = 13, Portal = 14, TradeNote = 15, ManaStone = 16, Plant = 17, BaseCooking = 18,
        BaseAlchemy = 19, BaseFletching = 20, CraftedCooking = 21, CraftedAlchemy = 22,
        CraftedFletching = 23, Player = 24, Vendor = 25, Door = 26, Corpse = 27, Lifestone = 28,
        HealingKit = 29, Lockpick = 30, WandStaffOrb = 31, Bundle = 32, Book = 33, Journal = 34,
        Sign = 35, Housing = 36, Npc = 37, Foci = 38, Salvage = 39, Ust = 40, Services = 41,
        Scroll = 42, NumObjectClasses = 43
    }

    // Decal's WorldObject as SortFlag reads it: absent keys read 0, 0.0, ""
    // and false.
    public class WorldObject
    {
        public int Id;
        public ObjectClass Class;
        public Dictionary<int, int> Longs = new Dictionary<int, int>();
        public Dictionary<int, double> Doubles = new Dictionary<int, double>();
        public Dictionary<int, string> Strings = new Dictionary<int, string>();
        public Dictionary<int, bool> Bools = new Dictionary<int, bool>();
        public List<int> Spells = new List<int>();

        public int Values(LongValueKey key) { int v; return Longs.TryGetValue((int)key, out v) ? v : 0; }
        public double Values(DoubleValueKey key) { double v; return Doubles.TryGetValue((int)key, out v) ? v : 0.0; }
        public string Values(StringValueKey key) { string v; return Strings.TryGetValue((int)key, out v) ? v : ""; }
        public bool Values(BoolValueKey key) { bool v; return Bools.TryGetValue((int)key, out v) && v; }
        public int SpellCount { get { return Spells.Count; } }
        public int Spell(int index) { return Spells[index]; }
        public ObjectClass ObjectClass { get { return Class; } }
    }

    public class Actions { public int CurrentSelection; }

    public class WorldFilter
    {
        public WorldObject this[int id] { get { return null; } }
    }

    public class CoreManager
    {
        public WorldFilter WorldFilter = new WorldFilter();
        public Actions Actions = new Actions();
    }
}

namespace mudsort
{
    using Decal.Adapter.Wrappers;

    public static class Globals
    {
        public static CoreManager Core = new CoreManager();
    }

    public static class Util
    {
        public static List<string> Logged = new List<string>();
        public static void Log(string message) { Logged.Add(message); }
        public static void LogError(Exception ex) { Logged.Add("error: " + ex.GetType().Name); }
        public static void WriteToChat(string message) { }
    }

    public static class Harness
    {
        static string Dir;

        public static int Main(string[] args)
        {
            Thread.CurrentThread.CurrentCulture = new CultureInfo("en-US");
            Thread.CurrentThread.CurrentUICulture = new CultureInfo("en-US");
            Dir = args[0];
            string tools = args[1];
            CodeTable();
            Alphanum(Path.Combine(tools, "alphanum_corpus.txt"));
            SortStrings();
            Cases(Path.Combine(Dir, "items.txt"));
            SortLists();
            return 0;
        }

        static void Write(string name, StringBuilder text)
        {
            File.WriteAllText(Path.Combine(Dir, name), text.ToString().Replace("\r\n", "\n"), new UTF8Encoding(false));
        }

        static string KindOf(SortFlag flag)
        {
            if (flag.key is MSStringValueKey) return "String";
            if (flag.key is MSLongValueKey) return "Long";
            if (flag.key is MSDoubleValueKey) return "Double";
            if (flag.key is MSBoolValueKey) return "Bool";
            return "Computed";
        }

        // Every flag SortFlag's static constructor registered, by code, and
        // the duplicate-code log lines (the keys that got no code).
        static void CodeTable()
        {
            var sb = new StringBuilder();
            var rows = new List<string>();
            foreach (SortFlag flag in SortFlag.sortedFlagList.Values)
            {
                string value = flag.key is string ? "-" : Convert.ToInt64(flag.key).ToString(CultureInfo.InvariantCulture);
                rows.Add(flag.code + "\t" + KindOf(flag) + "\t" + flag.name + "\t" + value
                    + "\t" + (SortFlag.CommonFlags.Contains(flag.name) ? "common" : "-"));
            }
            rows.Sort(StringComparer.Ordinal);
            foreach (string row in rows) sb.Append(row).Append('\n');
            foreach (string line in Util.Logged) sb.Append("log\t").Append(line).Append('\n');
            Write("flags_golden.txt", sb);
        }

        static string Unbracket(string line)
        {
            return line.Substring(1, line.Length - 2);
        }

        // The comparator over every pair of the corpus plus every printable
        // ASCII character: one row per string, '<', '=' or '>' per column.
        static void Alphanum(string corpusPath)
        {
            var corpus = new List<string>();
            foreach (string line in File.ReadAllLines(corpusPath, Encoding.UTF8))
                if (line.Length >= 2) corpus.Add(Unbracket(line));
            for (int c = 0x20; c < 0x7F; c++)
                if (!corpus.Contains(((char)c).ToString())) corpus.Add(((char)c).ToString());
            var cmp = new AlphanumComparator();
            var sb = new StringBuilder();
            foreach (string s in corpus) sb.Append("S\t[").Append(s).Append("]\n");
            foreach (string a in corpus)
            {
                sb.Append("M\t");
                foreach (string b in corpus)
                {
                    int r = cmp.Compare(a, b);
                    sb.Append(r < 0 ? '<' : r > 0 ? '>' : '=');
                }
                sb.Append('\n');
            }
            Write("alphanum_golden.txt", sb);
        }

        // PluginCore.createSortFlagListFromString, as written.
        static ArrayList Parse(string str)
        {
            ArrayList sortFlags = new ArrayList();
            foreach (SortFlag sf in SortFlag.sortedFlagList.Values) sf.descending = false;
            String[] sortKeys = str.Split(',');
            for (int i = 0; i < sortKeys.Length; i++)
            {
                try
                {
                    SortFlag sf = SortFlag.decode(sortKeys[i]);
                    sf.descending = sortKeys[i].Length == 3 && sortKeys[i].Substring(2, 1).Equals("-");
                    sortFlags.Add(sf);
                }
                catch (Exception e) { Util.LogError(e); }
            }
            return sortFlags;
        }

        static string Describe(ArrayList flags)
        {
            var parts = new List<string>();
            foreach (SortFlag f in flags) parts.Add(f.code + (f.descending ? "-" : ""));
            return string.Join(",", parts.ToArray());
        }

        // What each sort string decodes to: as typed into the edit box, and
        // upper-cased as `/ms set flags` passes it.
        static void SortStrings()
        {
            string[] inputs = {
                "OC,ES,CV,AL-,AB-,DG-,ED-,ML,MA,NM",
                "OC,EQ,CV,AL-,AB-,DB-,ED-,ML,MT,NM",
                "OC,EP,MA,AL-,MX-", "AL-,MX-", "BR-",
                "oc,al-,nm", "AL,AL-", "AL-,AL", " AL", "AL- ", "ALX", "AL-X", "AL--", "A", "",
                "OC,,NM", "ZZ,NM", "nm-,Oc",
            };
            var sb = new StringBuilder();
            foreach (string input in inputs)
            {
                sb.Append("[").Append(input).Append("]\t")
                  .Append(Describe(Parse(input))).Append('\t')
                  .Append(Describe(Parse(input.ToUpper()))).Append('\n');
            }
            Write("sort_string_golden.txt", sb);
        }

        // PluginCore's BUILDING_LIST state, as written: one pass per flag,
        // last flag first, then the optional reversal of the whole list.
        static ArrayList Sort(ArrayList sortFlags, ArrayList sortList, bool reverseSortList)
        {
            ArrayList queue = new ArrayList();
            ArrayList sortValueList = new ArrayList();
            for (int i = sortFlags.Count - 1; i >= 0; i--)
            {
                SortFlag sf = (SortFlag)sortFlags[i];
                foreach (WorldObject worldObject in sortList)
                {
                    String sortMetric = sf.valueOf(worldObject);
                    if (!sortValueList.Contains(sortMetric)) sortValueList.Add(sortMetric);
                }
                sortValueList.Sort(new AlphanumComparator());
                ArrayList newSortList = new ArrayList();
                if (sf.descending) sortValueList.Reverse();
                foreach (Object sortValue in sortValueList)
                {
                    foreach (WorldObject worldObject in sortList)
                    {
                        String sortMetric = sf.valueOf(worldObject);
                        if (sortMetric.Equals(sortValue)) newSortList.Add(worldObject);
                    }
                }
                sortList = newSortList;
                if (i == 0)
                {
                    if (reverseSortList) sortList.Reverse();
                    foreach (WorldObject worldObject in sortList) queue.Add(worldObject);
                }
            }
            return queue;
        }

        // ArrayList.Sort with the comparator over lists of distinct strings
        // in classes it calls equal ("x 1", "x 01", ...), so the order .NET's
        // unstable sort leaves them in shows: each list as given, then sorted.
        static void SortLists()
        {
            var pool = new List<string>();
            for (int k = 1; k <= 10; k++)
                for (int zeros = 0; zeros < 8; zeros++)
                    pool.Add("x " + new string('0', zeros) + k);
            var rnd = new Random(1);
            var sb = new StringBuilder();
            for (int n = 0; n < 200; n++)
            {
                var shuffled = new List<string>(pool);
                for (int i = shuffled.Count - 1; i > 0; i--)
                {
                    int j = rnd.Next(i + 1);
                    string t = shuffled[i]; shuffled[i] = shuffled[j]; shuffled[j] = t;
                }
                int length = 2 + rnd.Next(pool.Count - 1);
                var list = new ArrayList(shuffled.GetRange(0, length));
                sb.Append("I\t").Append(string.Join("\t", (string[])list.ToArray(typeof(string)))).Append('\n');
                list.Sort(new AlphanumComparator());
                sb.Append("O\t").Append(string.Join("\t", (string[])list.ToArray(typeof(string)))).Append('\n');
            }
            Write("dotnet_sort_golden.txt", sb);
        }

        static WorldObject ParseItem(string[] fields)
        {
            var wo = new WorldObject();
            wo.Id = int.Parse(fields[1], CultureInfo.InvariantCulture);
            wo.Class = (ObjectClass)int.Parse(fields[2], CultureInfo.InvariantCulture);
            for (int i = 3; i < fields.Length; i++)
            {
                string f = fields[i];
                if (f.StartsWith("P="))
                {
                    foreach (string s in f.Substring(2).Split(','))
                        if (s.Length > 0) wo.Spells.Add(int.Parse(s, CultureInfo.InvariantCulture));
                    continue;
                }
                int eq = f.IndexOf('=');
                int key = (int)long.Parse(f.Substring(1, eq - 1), CultureInfo.InvariantCulture);
                string v = f.Substring(eq + 1);
                switch (f[0])
                {
                    case 'L': wo.Longs[key] = int.Parse(v, CultureInfo.InvariantCulture); break;
                    case 'D': wo.Doubles[key] = double.Parse(v, CultureInfo.InvariantCulture); break;
                    case 'S': wo.Strings[key] = v; break;
                    case 'B': wo.Bools[key] = v == "True"; break;
                    default: throw new Exception("bad field " + f);
                }
            }
            return wo;
        }

        // items.txt: `case <name> <sort string> <reverse>` then `item` lines
        // then `end`. Writes every flag's value for every item, and each
        // case's sorted order.
        static void Cases(string path)
        {
            var values = new StringBuilder();
            var sorted = new StringBuilder();
            string name = null, sortString = null;
            bool reverse = false;
            ArrayList items = null;
            foreach (string raw in File.ReadAllLines(path, Encoding.UTF8))
            {
                if (raw.Length == 0 || raw.StartsWith("#")) continue;
                string[] fields = raw.Split('\t');
                if (fields[0] == "case")
                {
                    name = fields[1]; sortString = fields[2]; reverse = fields[3] == "1";
                    items = new ArrayList();
                }
                else if (fields[0] == "item")
                {
                    items.Add(ParseItem(fields));
                }
                else if (fields[0] == "end")
                {
                    foreach (WorldObject wo in items)
                    {
                        var rows = new List<string>();
                        foreach (SortFlag flag in SortFlag.sortedFlagList.Values)
                            rows.Add(flag.code + "=" + flag.valueOf(wo));
                        rows.Sort(StringComparer.Ordinal);
                        values.Append(name).Append('\t').Append(wo.Id).Append('\t')
                              .Append(string.Join("\t", rows.ToArray())).Append('\n');
                    }
                    var order = new List<string>();
                    foreach (WorldObject wo in Sort(Parse(sortString), items, reverse))
                        order.Add(wo.Id.ToString(CultureInfo.InvariantCulture));
                    sorted.Append(name).Append('\t').Append(string.Join(" ", order.ToArray())).Append('\n');
                }
            }
            Write("values_golden.txt", values);
            Write("sort_golden.txt", sorted);
        }
    }
}
