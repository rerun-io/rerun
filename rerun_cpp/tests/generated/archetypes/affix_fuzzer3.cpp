// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

#include "affix_fuzzer3.hpp"

#include <rerun/collection_adapter_builtins.hpp>

namespace rerun::archetypes {
    AffixFuzzer3 AffixFuzzer3::clear_fields() {
        auto archetype = AffixFuzzer3();
        archetype.fuzz2001 =
            ComponentBatch::empty<rerun::components::AffixFuzzer1>(Descriptor_fuzz2001)
                .value_or_throw();
        archetype.fuzz2002 =
            ComponentBatch::empty<rerun::components::AffixFuzzer2>(Descriptor_fuzz2002)
                .value_or_throw();
        archetype.fuzz2003 =
            ComponentBatch::empty<rerun::components::AffixFuzzer3>(Descriptor_fuzz2003)
                .value_or_throw();
        archetype.fuzz2004 =
            ComponentBatch::empty<rerun::components::AffixFuzzer4>(Descriptor_fuzz2004)
                .value_or_throw();
        archetype.fuzz2005 =
            ComponentBatch::empty<rerun::components::AffixFuzzer5>(Descriptor_fuzz2005)
                .value_or_throw();
        archetype.fuzz2006 =
            ComponentBatch::empty<rerun::components::AffixFuzzer6>(Descriptor_fuzz2006)
                .value_or_throw();
        archetype.fuzz2007 =
            ComponentBatch::empty<rerun::components::AffixFuzzer7>(Descriptor_fuzz2007)
                .value_or_throw();
        archetype.fuzz2008 =
            ComponentBatch::empty<rerun::components::AffixFuzzer8>(Descriptor_fuzz2008)
                .value_or_throw();
        archetype.fuzz2009 =
            ComponentBatch::empty<rerun::components::AffixFuzzer9>(Descriptor_fuzz2009)
                .value_or_throw();
        archetype.fuzz2010 =
            ComponentBatch::empty<rerun::components::AffixFuzzer10>(Descriptor_fuzz2010)
                .value_or_throw();
        archetype.fuzz2011 =
            ComponentBatch::empty<rerun::components::AffixFuzzer11>(Descriptor_fuzz2011)
                .value_or_throw();
        archetype.fuzz2012 =
            ComponentBatch::empty<rerun::components::AffixFuzzer12>(Descriptor_fuzz2012)
                .value_or_throw();
        archetype.fuzz2013 =
            ComponentBatch::empty<rerun::components::AffixFuzzer13>(Descriptor_fuzz2013)
                .value_or_throw();
        archetype.fuzz2014 =
            ComponentBatch::empty<rerun::components::AffixFuzzer14>(Descriptor_fuzz2014)
                .value_or_throw();
        archetype.fuzz2015 =
            ComponentBatch::empty<rerun::components::AffixFuzzer15>(Descriptor_fuzz2015)
                .value_or_throw();
        archetype.fuzz2016 =
            ComponentBatch::empty<rerun::components::AffixFuzzer16>(Descriptor_fuzz2016)
                .value_or_throw();
        archetype.fuzz2017 =
            ComponentBatch::empty<rerun::components::AffixFuzzer17>(Descriptor_fuzz2017)
                .value_or_throw();
        archetype.fuzz2018 =
            ComponentBatch::empty<rerun::components::AffixFuzzer18>(Descriptor_fuzz2018)
                .value_or_throw();
        return archetype;
    }

    Collection<ComponentColumn> AffixFuzzer3::columns(const Collection<uint32_t>& lengths_) {
        std::vector<ComponentColumn> columns;
        columns.reserve(19);
        if (fuzz2001.has_value()) {
            columns.push_back(fuzz2001.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2002.has_value()) {
            columns.push_back(fuzz2002.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2003.has_value()) {
            columns.push_back(fuzz2003.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2004.has_value()) {
            columns.push_back(fuzz2004.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2005.has_value()) {
            columns.push_back(fuzz2005.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2006.has_value()) {
            columns.push_back(fuzz2006.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2007.has_value()) {
            columns.push_back(fuzz2007.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2008.has_value()) {
            columns.push_back(fuzz2008.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2009.has_value()) {
            columns.push_back(fuzz2009.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2010.has_value()) {
            columns.push_back(fuzz2010.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2011.has_value()) {
            columns.push_back(fuzz2011.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2012.has_value()) {
            columns.push_back(fuzz2012.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2013.has_value()) {
            columns.push_back(fuzz2013.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2014.has_value()) {
            columns.push_back(fuzz2014.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2015.has_value()) {
            columns.push_back(fuzz2015.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2016.has_value()) {
            columns.push_back(fuzz2016.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2017.has_value()) {
            columns.push_back(fuzz2017.value().partitioned(lengths_).value_or_throw());
        }
        if (fuzz2018.has_value()) {
            columns.push_back(fuzz2018.value().partitioned(lengths_).value_or_throw());
        }
        columns.push_back(
            ComponentColumn::from_indicators<AffixFuzzer3>(static_cast<uint32_t>(lengths_.size()))
                .value_or_throw()
        );
        return columns;
    }

    Collection<ComponentColumn> AffixFuzzer3::columns() {
        if (fuzz2001.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2001.value().length(), 1));
        }
        if (fuzz2002.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2002.value().length(), 1));
        }
        if (fuzz2003.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2003.value().length(), 1));
        }
        if (fuzz2004.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2004.value().length(), 1));
        }
        if (fuzz2005.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2005.value().length(), 1));
        }
        if (fuzz2006.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2006.value().length(), 1));
        }
        if (fuzz2007.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2007.value().length(), 1));
        }
        if (fuzz2008.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2008.value().length(), 1));
        }
        if (fuzz2009.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2009.value().length(), 1));
        }
        if (fuzz2010.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2010.value().length(), 1));
        }
        if (fuzz2011.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2011.value().length(), 1));
        }
        if (fuzz2012.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2012.value().length(), 1));
        }
        if (fuzz2013.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2013.value().length(), 1));
        }
        if (fuzz2014.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2014.value().length(), 1));
        }
        if (fuzz2015.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2015.value().length(), 1));
        }
        if (fuzz2016.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2016.value().length(), 1));
        }
        if (fuzz2017.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2017.value().length(), 1));
        }
        if (fuzz2018.has_value()) {
            return columns(std::vector<uint32_t>(fuzz2018.value().length(), 1));
        }
        return Collection<ComponentColumn>();
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::AffixFuzzer3>::serialize(
        const archetypes::AffixFuzzer3& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(19);

        if (archetype.fuzz2001.has_value()) {
            cells.push_back(archetype.fuzz2001.value());
        }
        if (archetype.fuzz2002.has_value()) {
            cells.push_back(archetype.fuzz2002.value());
        }
        if (archetype.fuzz2003.has_value()) {
            cells.push_back(archetype.fuzz2003.value());
        }
        if (archetype.fuzz2004.has_value()) {
            cells.push_back(archetype.fuzz2004.value());
        }
        if (archetype.fuzz2005.has_value()) {
            cells.push_back(archetype.fuzz2005.value());
        }
        if (archetype.fuzz2006.has_value()) {
            cells.push_back(archetype.fuzz2006.value());
        }
        if (archetype.fuzz2007.has_value()) {
            cells.push_back(archetype.fuzz2007.value());
        }
        if (archetype.fuzz2008.has_value()) {
            cells.push_back(archetype.fuzz2008.value());
        }
        if (archetype.fuzz2009.has_value()) {
            cells.push_back(archetype.fuzz2009.value());
        }
        if (archetype.fuzz2010.has_value()) {
            cells.push_back(archetype.fuzz2010.value());
        }
        if (archetype.fuzz2011.has_value()) {
            cells.push_back(archetype.fuzz2011.value());
        }
        if (archetype.fuzz2012.has_value()) {
            cells.push_back(archetype.fuzz2012.value());
        }
        if (archetype.fuzz2013.has_value()) {
            cells.push_back(archetype.fuzz2013.value());
        }
        if (archetype.fuzz2014.has_value()) {
            cells.push_back(archetype.fuzz2014.value());
        }
        if (archetype.fuzz2015.has_value()) {
            cells.push_back(archetype.fuzz2015.value());
        }
        if (archetype.fuzz2016.has_value()) {
            cells.push_back(archetype.fuzz2016.value());
        }
        if (archetype.fuzz2017.has_value()) {
            cells.push_back(archetype.fuzz2017.value());
        }
        if (archetype.fuzz2018.has_value()) {
            cells.push_back(archetype.fuzz2018.value());
        }
        {
            auto result = ComponentBatch::from_indicator<AffixFuzzer3>();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
